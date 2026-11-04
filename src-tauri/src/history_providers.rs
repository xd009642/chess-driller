use crate::{
    history::{Account, Coverage, Game, Provider, GAME_LIMIT, WINDOW_MS},
    history_review, provider_http,
};
use anyhow::{bail, Context};
use reqwest::{blocking::Client, Url};
use serde_json::Value;
use std::{
    collections::{BTreeMap, HashSet},
    io::{BufRead, BufReader, Read},
    sync::{
        atomic::{AtomicBool, Ordering},
        Mutex,
    },
};

const BODY_LIMIT: u64 = 16 * 1024 * 1024;
const SCAN_LIMIT: usize = 2000;
const LINE_LIMIT: u64 = 256 * 1024;
struct Cached {
    body: Vec<u8>,
    etag: Option<String>,
    modified: Option<String>,
}
static ARCHIVES: Mutex<BTreeMap<String, Cached>> = Mutex::new(BTreeMap::new());

pub struct Download {
    pub games: Vec<Game>,
    pub coverage: Coverage,
    pub error: Option<String>,
}

fn check_cancel(cancel: &AtomicBool) -> anyhow::Result<()> {
    if cancel.load(Ordering::Relaxed) {
        bail!("Refresh cancelled");
    }
    Ok(())
}

fn text<'a>(value: &'a Value, pointer: &str) -> anyhow::Result<&'a str> {
    value
        .pointer(pointer)
        .and_then(Value::as_str)
        .with_context(|| format!("Missing {pointer}"))
}
fn timestamp(value: &Value, pointer: &str) -> anyhow::Result<i64> {
    value
        .pointer(pointer)
        .and_then(Value::as_i64)
        .filter(|n| *n > 0)
        .with_context(|| format!("Invalid {pointer} timestamp"))
}

pub fn game_url(provider: Provider, url: &str) -> anyhow::Result<Url> {
    let parsed = Url::parse(url)?;
    let host = match provider {
        Provider::ChessCom => "www.chess.com",
        Provider::Lichess => "lichess.org",
    };
    if parsed.scheme() != "https"
        || parsed.host_str() != Some(host)
        || parsed.port().is_some()
        || !parsed.username().is_empty()
        || parsed.password().is_some()
    {
        bail!("Invalid provider game URL");
    }
    let path = parsed.path();
    let valid = match provider {
        Provider::ChessCom => path
            .strip_prefix("/game/live/")
            .or_else(|| path.strip_prefix("/live/game/"))
            .is_some_and(|id| !id.is_empty() && id.bytes().all(|b| b.is_ascii_digit())),
        Provider::Lichess => path
            .strip_prefix('/')
            .is_some_and(|id| id.len() == 8 && id.bytes().all(|b| b.is_ascii_alphanumeric())),
    };
    if !valid || parsed.query().is_some() || parsed.fragment().is_some() {
        bail!("Invalid completed-game URL");
    }
    Ok(parsed)
}

fn normalize(account: &Account, value: &Value) -> anyhow::Result<Game> {
    let (id, url, white, black, played_ms, started_ms, pgn, time_control, result) =
        match account.provider {
            Provider::ChessCom => {
                if text(value, "/rules")? != "chess" {
                    bail!("Unsupported variant");
                }
                if text(value, "/time_class")? == "daily" {
                    bail!("Daily/correspondence game");
                }
                let white = text(value, "/white/username")?.to_owned();
                let black = text(value, "/black/username")?.to_owned();
                let wr = text(value, "/white/result")?;
                let br = text(value, "/black/result")?;
                let result = if wr == "win" {
                    "1-0"
                } else if br == "win" {
                    "0-1"
                } else if [
                    "agreed",
                    "repetition",
                    "stalemate",
                    "insufficient",
                    "50move",
                    "timevsinsufficient",
                ]
                .contains(&wr)
                {
                    "1/2-1/2"
                } else {
                    bail!("Unsupported or incomplete result");
                };
                let url = text(value, "/url")?.to_owned();
                game_url(account.provider, &url)?;
                let id = url
                    .rsplit('/')
                    .next()
                    .context("Missing game ID")?
                    .to_owned();
                (
                    id,
                    url,
                    white,
                    black,
                    timestamp(value, "/end_time")?
                        .checked_mul(1000)
                        .context("Timestamp overflow")?,
                    value
                        .get("start_time")
                        .and_then(Value::as_i64)
                        .and_then(|n| n.checked_mul(1000)),
                    text(value, "/pgn")?.to_owned(),
                    text(value, "/time_control")?.to_owned(),
                    result.to_owned(),
                )
            }
            Provider::Lichess => {
                if text(value, "/variant")? != "standard"
                    || value.get("initialFen").is_some_and(|v| !v.is_null())
                {
                    bail!("Unsupported variant or starting position");
                }
                if text(value, "/speed")? == "correspondence" {
                    bail!("Daily/correspondence game");
                }
                if ![
                    "mate",
                    "resign",
                    "stalemate",
                    "timeout",
                    "draw",
                    "outoftime",
                    "cheat",
                    "variantEnd",
                ]
                .contains(&text(value, "/status")?)
                {
                    bail!("Game is not completed");
                }
                let id = text(value, "/id")?.to_owned();
                let url = format!("https://lichess.org/{id}");
                game_url(account.provider, &url)?;
                let name = |color: &str| {
                    value
                        .pointer(&format!("/players/{color}/user/name"))
                        .and_then(Value::as_str)
                        .unwrap_or("Anonymous")
                        .to_owned()
                };
                let result = match value.get("winner").and_then(Value::as_str) {
                    Some("white") => "1-0",
                    Some("black") => "0-1",
                    None => "1/2-1/2",
                    _ => bail!("Invalid winner"),
                };
                (
                    id,
                    url,
                    name("white"),
                    name("black"),
                    timestamp(value, "/lastMoveAt")?,
                    Some(timestamp(value, "/createdAt")?),
                    text(value, "/pgn")?.to_owned(),
                    text(value, "/speed")?.to_owned(),
                    result.to_owned(),
                )
            }
        };
    let identifies = |side: &str, name: &str| match account.provider {
        Provider::ChessCom => name.eq_ignore_ascii_case(&account.username),
        Provider::Lichess => value
            .pointer(&format!("/players/{side}/user/id"))
            .or_else(|| value.pointer(&format!("/players/{side}/user/name")))
            .and_then(Value::as_str)
            .is_some_and(|id| id.eq_ignore_ascii_case(&account.username)),
    };
    let color = match (identifies("white", &white), identifies("black", &black)) {
        (true, false) => "white",
        (false, true) => "black",
        _ => bail!("Account/player identity mismatch"),
    };
    let moves = history_review::parse(&pgn)?;
    Ok(Game {
        provider: account.provider,
        id,
        account: account.username.clone(),
        url,
        started_ms,
        played_ms,
        white,
        black,
        color: color.into(),
        result,
        time_control,
        pgn,
        moves,
    })
}

fn accept(account: &Account, value: &Value, download: &mut Download, seen: &mut HashSet<String>) {
    download.coverage.scanned += 1;
    match normalize(account, value) {
        Ok(game) => {
            if game.played_ms < download.coverage.since_ms
                || game.played_ms > download.coverage.until_ms
            {
                return;
            }
            if seen.insert(game.id.clone()) {
                download.games.push(game);
            }
        }
        Err(error) => {
            // Keep reasons bounded and useful; individual PGN errors can contain arbitrary text.
            let reason = error.to_string();
            let reason = if reason.starts_with("Illegal mainline") {
                "Illegal mainline move"
            } else if reason.starts_with("Missing ") || reason.starts_with("Invalid ") {
                "Malformed provider record"
            } else {
                &reason
            };
            *download
                .coverage
                .skipped
                .entry(reason.to_owned())
                .or_default() += 1;
        }
    }
}

fn archive(client: &Client, url: &str, cancel: &AtomicBool) -> anyhow::Result<Value> {
    let mut guard = provider_http::gate(false)?;
    check_cancel(cancel)?;
    let mut cache = ARCHIVES
        .lock()
        .map_err(|_| anyhow::anyhow!("Archive cache unavailable"))?;
    let mut request = client.get(url);
    if let Some(old) = cache.get(url) {
        if let Some(etag) = &old.etag {
            request = request.header("If-None-Match", etag);
        }
        if let Some(modified) = &old.modified {
            request = request.header("If-Modified-Since", modified);
        }
    }
    let response = request
        .send()
        .context("Cannot reach Chess.com; check your connection")?;
    if response.status() == reqwest::StatusCode::NOT_MODIFIED {
        return Ok(serde_json::from_slice(
            &cache.get(url).context("Unexpected HTTP 304")?.body,
        )?);
    }
    provider_http::status(&response, &mut guard)?;
    if response.status().is_redirection() {
        bail!("Unexpected provider redirect");
    }
    let header = |key: &str| {
        response
            .headers()
            .get(key)
            .and_then(|v| v.to_str().ok())
            .map(str::to_owned)
    };
    let etag = header("etag");
    let modified = header("last-modified");
    let cacheable = !header("cache-control")
        .unwrap_or_default()
        .contains("no-store");
    let mut body = Vec::new();
    response.take(BODY_LIMIT + 1).read_to_end(&mut body)?;
    check_cancel(cancel)?;
    if body.len() as u64 > BODY_LIMIT {
        bail!("16 MiB archive response limit reached; coverage is truncated");
    }
    let value = serde_json::from_slice(&body).context("Malformed Chess.com archive")?;
    if cacheable {
        if cache.len() >= 5 {
            cache.clear();
        }
        cache.insert(
            url.to_owned(),
            Cached {
                body,
                etag,
                modified,
            },
        );
    } else {
        cache.remove(url);
    }
    Ok(value)
}

pub fn fetch(account: &Account, cancel: &AtomicBool, mut progress: impl FnMut(String)) -> Download {
    let now = chrono::Utc::now().timestamp_millis();
    let mut download = Download {
        games: vec![],
        coverage: Coverage {
            since_ms: now - WINDOW_MS,
            until_ms: now,
            ..Coverage::default()
        },
        error: None,
    };
    let work = (|| -> anyhow::Result<()> {
        let client = provider_http::client()?;
        let mut seen = HashSet::new();
        match account.provider {
            Provider::ChessCom => {
                let base = format!(
                    "https://api.chess.com/pub/player/{}/games",
                    account.username
                );
                let index = archive(&client, &format!("{base}/archives"), cancel)?;
                let mut urls: Vec<_> = index
                    .get("archives")
                    .and_then(Value::as_array)
                    .context("Missing archives")?
                    .iter()
                    .map(|v| v.as_str().context("Invalid archive URL"))
                    .collect::<anyhow::Result<_>>()?;
                urls.sort_unstable();
                urls.dedup();
                let first_month =
                    chrono::DateTime::from_timestamp_millis(download.coverage.since_ms)
                        .context("Invalid date")?
                        .format("%Y/%m")
                        .to_string();
                let last_month = chrono::Utc::now().format("%Y/%m").to_string();
                for url in urls.into_iter().rev() {
                    check_cancel(cancel)?;
                    let suffix = url
                        .strip_prefix(&format!("{base}/"))
                        .context("Unexpected archive host/path")?;
                    if suffix.len() != 7
                        || chrono::NaiveDate::parse_from_str(&format!("{suffix}/01"), "%Y/%m/%d")
                            .is_err()
                    {
                        bail!("Invalid archive month");
                    }
                    if suffix < first_month.as_str() || suffix > last_month.as_str() {
                        continue;
                    }
                    progress(format!("Chess.com: reading {suffix}"));
                    let data = archive(&client, url, cancel)?;
                    let mut games: Vec<_> = data
                        .get("games")
                        .and_then(Value::as_array)
                        .context("Missing archive games")?
                        .iter()
                        .collect();
                    games.sort_by(|a, b| {
                        b.get("end_time")
                            .and_then(Value::as_i64)
                            .cmp(&a.get("end_time").and_then(Value::as_i64))
                            .then_with(|| {
                                a.get("url")
                                    .and_then(Value::as_str)
                                    .cmp(&b.get("url").and_then(Value::as_str))
                            })
                    });
                    for value in games {
                        check_cancel(cancel)?;
                        accept(account, value, &mut download, &mut seen);
                        if download.coverage.scanned >= SCAN_LIMIT
                            || download.games.len() >= GAME_LIMIT
                        {
                            break;
                        }
                    }
                    if download.coverage.scanned >= SCAN_LIMIT || download.games.len() >= GAME_LIMIT
                    {
                        break;
                    }
                }
            }
            Provider::Lichess => {
                let mut guard = provider_http::gate(true)?;
                check_cancel(cancel)?;
                progress("Lichess: streaming recent games".into());
                let response = client
                    .get(format!(
                        "https://lichess.org/api/games/user/{}",
                        account.username
                    ))
                    .header("Accept", "application/x-ndjson")
                    .query(&[
                        ("since", download.coverage.since_ms.to_string()),
                        ("until", now.to_string()),
                        ("max", SCAN_LIMIT.to_string()),
                        ("sort", "dateDesc".into()),
                        ("ongoing", "false".into()),
                        ("finished", "true".into()),
                        ("moves", "true".into()),
                        ("pgnInJson", "true".into()),
                        ("clocks", "false".into()),
                        ("evals", "false".into()),
                        ("opening", "false".into()),
                    ])
                    .send()
                    .context("Cannot reach Lichess; check your connection")?;
                provider_http::status(&response, &mut guard)?;
                if response.status().is_redirection() {
                    bail!("Unexpected provider redirect");
                }
                let mut reader = BufReader::new(response.take(BODY_LIMIT + 1));
                let mut bytes = 0;
                loop {
                    check_cancel(cancel)?;
                    let mut line = String::new();
                    let count = (&mut reader).take(LINE_LIMIT + 1).read_line(&mut line)?;
                    if count == 0 {
                        break;
                    }
                    bytes += count as u64;
                    if count as u64 > LINE_LIMIT || bytes > BODY_LIMIT {
                        bail!("Response size limit reached; coverage is truncated");
                    }
                    if line.trim().is_empty() {
                        continue;
                    }
                    let value: Value =
                        serde_json::from_str(&line).context("Malformed Lichess NDJSON")?;
                    accept(account, &value, &mut download, &mut seen);
                    if download.coverage.scanned >= SCAN_LIMIT || download.games.len() >= GAME_LIMIT
                    {
                        break;
                    }
                }
                // One bounded export avoids unsafe timestamp pagination: equal-time games
                // cannot be traversed reliably with the API's date-only cursor.
                download.coverage.limits.push("Lichess filters and orders by game start; displayed dates are last-move times, not resignation/timeout times. Games started before the window are excluded; equal-time ties at a limit may be incomplete.".into());
            }
        }
        Ok(())
    })();
    if let Err(error) = work {
        download.error = Some(format!("{error:#}"));
    }
    if download.coverage.scanned >= SCAN_LIMIT {
        download.coverage.limits.push(
            "2,000-record scan limit reached; older games and equal-timestamp ties may be missing"
                .into(),
        );
    }
    if download.games.len() >= GAME_LIMIT {
        download
            .coverage
            .limits
            .push("Latest 100 eligible games retained".into());
    }
    download
        .games
        .sort_by(|a, b| b.played_ms.cmp(&a.played_ms).then(a.id.cmp(&b.id)));
    download.games.truncate(GAME_LIMIT);
    download.coverage.eligible = download.games.len();
    download
}

#[cfg(test)]
mod tests {
    use super::*;
    fn account(provider: Provider) -> Account {
        Account {
            provider,
            username: "alice".into(),
            last_success_ms: None,
            attempted_ms: None,
            error: None,
            coverage: Coverage::default(),
            games: vec![],
        }
    }
    // Minimal reviewed fixtures follow the official archive and GameJson schemas.
    fn chess() -> Value {
        serde_json::json!({"url":"https://www.chess.com/game/live/123", "rules":"chess", "time_class":"blitz", "time_control":"180", "end_time":1700000000,
        "white":{"username":"ALIce","result":"win"}, "black":{"username":"Bob","result":"resigned"}, "pgn":"1. e4 e5 1-0", "extra":"ignored"})
    }
    fn lichess() -> Value {
        serde_json::json!({"id":"abcd1234", "variant":"standard", "speed":"rapid", "createdAt":1700000000000i64, "lastMoveAt":1700000100000i64, "status":"resign", "winner":"white", "players":{"white":{"user":{"name":"Bob"}}, "black":{"user":{"name":"ALICE"}}}, "pgn":"1. e4 e5 1-0", "extra":"ignored"})
    }

    /// Provider units and player metadata govern date/colour, independently of
    /// winner and username case. Unrelated provider fields remain permissible.
    #[test]
    fn normalization_uses_identity_and_explicit_time_units() {
        let game = normalize(&account(Provider::ChessCom), &chess()).expect("Archive fixture");
        assert_eq!(game.color, "white");
        assert_eq!(game.played_ms, 1700000000000);
        let game = normalize(&account(Provider::Lichess), &lichess()).expect("Export fixture");
        assert_eq!(game.color, "black");
        assert_eq!(game.result, "1-0");
        assert_eq!(game.played_ms, 1700000100000);
        assert_eq!(game.started_ms, Some(1700000000000));
    }

    /// Unsupported records and invalid identities are skips, never departures.
    /// Standard live games remain accepted regardless of rated/casual metadata.
    #[test]
    fn skips_unsupported_and_invalid_records() {
        for (field, value) in [
            ("speed", "correspondence"),
            ("variant", "atomic"),
            ("status", "started"),
            ("pgn", "1. e4 e5 2. Ke4 *"),
        ] {
            let mut data = lichess();
            data[field] = value.into();
            assert!(normalize(&account(Provider::Lichess), &data).is_err());
        }
        let mut wrong = account(Provider::Lichess);
        wrong.username = "charlie".into();
        assert!(normalize(&wrong, &lichess()).is_err());
        let mut daily = chess();
        daily["time_class"] = "daily".into();
        assert!(normalize(&account(Provider::ChessCom), &daily).is_err());
        for rated in [true, false] {
            let mut data = lichess();
            data["rated"] = rated.into();
            normalize(&account(Provider::Lichess), &data).expect("Rated and casual");
        }
    }

    /// Stable IDs deduplicate overlapping boundaries, while identical moves in
    /// different games stay distinct. Skips do not consume the eligible budget.
    #[test]
    fn record_budget_and_timestamp_boundaries() {
        let account = account(Provider::Lichess);
        let mut download = Download {
            games: vec![],
            coverage: Coverage {
                since_ms: 1700000100000,
                until_ms: 1700000100001,
                ..Coverage::default()
            },
            error: None,
        };
        let mut seen = HashSet::new();
        let mut value = lichess();
        for _ in 0..2 {
            accept(&account, &value, &mut download, &mut seen);
        }
        value["id"] = "efgh5678".into();
        value["lastMoveAt"] = 1700000100001i64.into();
        accept(&account, &value, &mut download, &mut seen);
        value["id"] = "ijkl9012".into();
        value["lastMoveAt"] = 1700000099999i64.into();
        accept(&account, &value, &mut download, &mut seen);
        value["variant"] = "atomic".into();
        accept(&account, &value, &mut download, &mut seen);
        assert_eq!(download.games.len(), 2);
        assert_eq!(download.coverage.scanned, 5);
        assert_eq!(
            download.coverage.skipped["Unsupported variant or starting position"],
            1
        );
    }

    /// Provider-supplied URLs cannot open arbitrary hosts, schemes or commands.
    /// Both documented Chess.com live-game path forms remain accepted.
    #[test]
    fn restricts_game_links() {
        for url in [
            "http://lichess.org/abcd1234",
            "https://lichess.org.evil.test/abcd1234",
            "https://user@lichess.org/abcd1234",
            "https://lichess.org/abcd1234?x=1",
            "https://lichess.org/@/alice",
        ] {
            assert!(game_url(Provider::Lichess, url).is_err());
        }
        for url in [
            "https://www.chess.com/game/live/123",
            "https://www.chess.com/live/game/123",
        ] {
            game_url(Provider::ChessCom, url).expect("Public game URL");
        }
    }
    /// Conditional requests must reuse the verified body on 304 and must surface
    /// provider errors. The real HTTP client talks to a local HTTP fixture server.
    #[test]
    fn conditional_archive_requests_and_http_errors() {
        use std::{io::Write, net::TcpListener, thread};
        let listener = TcpListener::bind("127.0.0.1:0").expect("Bind fixture server");
        let url = format!(
            "http://{}/archive",
            listener.local_addr().expect("Server address")
        );
        let server = thread::spawn(move || {
            for round in 0..3 {
                let (mut stream, _) = listener.accept().expect("Accept request");
                stream
                    .set_read_timeout(Some(std::time::Duration::from_secs(5)))
                    .expect("Read deadline");
                let mut request = String::new();
                let mut reader = BufReader::new(&stream);
                loop {
                    let mut line = String::new();
                    reader.read_line(&mut line).expect("Read request");
                    if line == "\r\n" {
                        break;
                    }
                    request.push_str(&line);
                }
                let response = match round {
                    0 => "HTTP/1.1 200 OK\r\nContent-Length: 12\r\nETag: \"fixture\"\r\nConnection: close\r\n\r\n{\"games\":[]}",
                    1 => {
                        assert!(request.to_ascii_lowercase().contains("if-none-match: \"fixture\""));
                        "HTTP/1.1 304 Not Modified\r\nConnection: close\r\n\r\n"
                    }
                    _ => "HTTP/1.1 404 Not Found\r\nContent-Length: 0\r\nConnection: close\r\n\r\n",
                };
                stream
                    .write_all(response.as_bytes())
                    .expect("Write response");
            }
        });
        let client = provider_http::client().expect("HTTP client");
        let cancel = AtomicBool::new(false);
        for _ in 0..2 {
            assert_eq!(
                archive(&client, &url, &cancel).expect("Read archive"),
                serde_json::json!({"games": []})
            );
        }
        assert!(archive(&client, &url, &cancel)
            .expect_err("404 must fail")
            .to_string()
            .contains("not found"));
        server.join().expect("Fixture server completed");
    }

    /// Explicit manual check; public usernames are supplied by the operator.
    #[test]
    #[ignore = "requires network and CHESS_DRILLER_SMOKE_CHESS_COM / CHESS_DRILLER_SMOKE_LICHESS"]
    fn live_history_providers() {
        for (provider, variable) in [
            (Provider::ChessCom, "CHESS_DRILLER_SMOKE_CHESS_COM"),
            (Provider::Lichess, "CHESS_DRILLER_SMOKE_LICHESS"),
        ] {
            let mut account = account(provider);
            account.username =
                std::env::var(variable).expect("Set public username for smoke check");
            let result = fetch(&account, &AtomicBool::new(false), |message| {
                eprintln!("{message}")
            });
            eprintln!(
                "{provider:?}: {} games, {} scanned, skips: {:?}, error: {:?}",
                result.games.len(),
                result.coverage.scanned,
                result.coverage.skipped,
                result.error
            );
            assert!(
                result.error.is_none(),
                "Provider request failed: {:?}",
                result.error
            );
        }
    }
}
