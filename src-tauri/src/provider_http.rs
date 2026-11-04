//! A shared Lichess gate covers the entire response body, including explorer requests.
use anyhow::{bail, Context};
use reqwest::{
    blocking::{Client, Response},
    StatusCode,
};
use std::{
    sync::{Mutex, MutexGuard},
    time::{Duration, Instant},
};

static LICHESS: Mutex<Option<Instant>> = Mutex::new(None);
static CHESS_COM: Mutex<Option<Instant>> = Mutex::new(None);

pub fn gate(lichess: bool) -> anyhow::Result<MutexGuard<'static, Option<Instant>>> {
    let guard = if lichess { &LICHESS } else { &CHESS_COM }
        .lock()
        .map_err(|_| anyhow::anyhow!("Provider request state unavailable"))?;
    if let Some(until) = *guard {
        if until > Instant::now() {
            bail!(
                "Rate limited; retry in {} seconds",
                (until - Instant::now()).as_secs() + 1
            );
        }
    }
    Ok(guard)
}

pub fn status(response: &Response, guard: &mut Option<Instant>) -> anyhow::Result<()> {
    if response.status() == StatusCode::TOO_MANY_REQUESTS {
        let delay = response
            .headers()
            .get("retry-after")
            .and_then(|v| v.to_str().ok())
            .and_then(|v| {
                v.parse::<u64>().ok().or_else(|| {
                    chrono::DateTime::parse_from_rfc2822(v).ok().map(|date| {
                        (date.timestamp() - chrono::Utc::now().timestamp()).max(0) as u64
                    })
                })
            })
            .unwrap_or(60)
            .max(60);
        *guard = Instant::now().checked_add(Duration::from_secs(delay));
        bail!("Provider rate limit reached; retry after {delay} seconds");
    }
    if response.status() == StatusCode::NOT_FOUND {
        bail!("Public account not found; check the saved username");
    }
    response
        .error_for_status_ref()
        .context("Provider request failed")?;
    Ok(())
}

pub fn client() -> anyhow::Result<Client> {
    Ok(Client::builder()
        .timeout(Duration::from_secs(30))
        .connect_timeout(Duration::from_secs(10))
        .user_agent("chess-driller/desktop (public game history)")
        // API URLs are canonical. Reject redirects rather than following an untrusted host.
        .redirect(reqwest::redirect::Policy::none())
        .build()?)
}
