use crate::history_review::PlayedMove;
use anyhow::{bail, Context};
use serde::{Deserialize, Serialize};
use std::{
    collections::BTreeMap,
    fs,
    path::PathBuf,
    sync::{
        atomic::{AtomicBool, Ordering},
        Arc,
    },
};

pub const WINDOW_MS: i64 = 90 * 24 * 60 * 60 * 1000;
pub const GAME_LIMIT: usize = 100;

#[derive(Clone, Copy, Debug, Deserialize, Serialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum Provider {
    ChessCom,
    Lichess,
}

#[derive(Clone, Deserialize, Serialize)]
pub struct Game {
    pub provider: Provider,
    pub id: String,
    pub account: String,
    pub url: String,
    pub started_ms: Option<i64>,
    // Lichess exposes the last move time, not the time of resignation/timeout.
    pub played_ms: i64,
    pub white: String,
    pub black: String,
    pub color: String,
    pub result: String,
    pub time_control: String,
    pub pgn: String,
    pub moves: Vec<PlayedMove>,
}

#[derive(Clone, Default, Deserialize, Serialize)]
pub struct Coverage {
    pub since_ms: i64,
    pub until_ms: i64,
    pub scanned: usize,
    pub eligible: usize,
    pub skipped: BTreeMap<String, usize>,
    pub limits: Vec<String>,
}

#[derive(Clone, Deserialize, Serialize)]
pub struct Account {
    pub provider: Provider,
    pub username: String,
    pub last_success_ms: Option<i64>,
    pub attempted_ms: Option<i64>,
    pub error: Option<String>,
    pub coverage: Coverage,
    pub games: Vec<Game>,
}

#[derive(Clone, Deserialize, Serialize)]
pub struct Saved {
    version: u32,
    pub accounts: Vec<Account>,
}

pub struct Store {
    pub saved: Saved,
    pub error: Option<String>,
    path: PathBuf,
    pub generation: u64,
    pub data_revision: u64,
    pub job: Option<Arc<AtomicBool>>,
    pub progress: String,
}

#[derive(Serialize)]
pub struct AccountView {
    pub provider: Provider,
    pub username: String,
    pub last_success_ms: Option<i64>,
    pub attempted_ms: Option<i64>,
    pub error: Option<String>,
    pub coverage: Coverage,
    pub cached_count: usize,
    pub oldest_ms: Option<i64>,
    pub newest_ms: Option<i64>,
}

#[derive(Serialize)]
pub struct View {
    pub accounts: Vec<AccountView>,
    pub revision: u64,
    pub error: Option<String>,
    pub busy: bool,
    pub progress: String,
}

impl Store {
    pub fn load(directory: PathBuf) -> Self {
        let path = directory.join("history.json");
        let loaded = (|| -> anyhow::Result<Saved> {
            if !path.exists() {
                return Ok(Saved {
                    version: 1,
                    accounts: vec![],
                });
            }
            let saved: Saved = serde_json::from_slice(&fs::read(&path)?)?;
            if saved.version != 1 {
                bail!("Unsupported history version {}", saved.version);
            }
            Ok(saved)
        })();
        let (saved, error) = match loaded {
            Ok(saved) => (saved, None),
            Err(error) => (Saved { version: 1, accounts: vec![] }, Some(format!("Cannot load {}: {error:#}. The file has been preserved; restore it before saving.", path.display()))),
        };
        Self {
            saved,
            error,
            path,
            generation: 0,
            data_revision: 0,
            job: None,
            progress: String::new(),
        }
    }

    pub fn view(&self) -> View {
        View {
            accounts: self
                .saved
                .accounts
                .iter()
                .map(|account| AccountView {
                    provider: account.provider,
                    username: account.username.clone(),
                    last_success_ms: account.last_success_ms,
                    attempted_ms: account.attempted_ms,
                    error: account.error.clone(),
                    coverage: account.coverage.clone(),
                    cached_count: account.games.len(),
                    oldest_ms: account.games.iter().map(|g| g.played_ms).min(),
                    newest_ms: account.games.iter().map(|g| g.played_ms).max(),
                })
                .collect(),
            revision: self.data_revision,
            error: self.error.clone(),
            busy: self.job.is_some(),
            progress: self.progress.clone(),
        }
    }

    fn persist(&mut self, saved: Saved) -> anyhow::Result<()> {
        if let Some(error) = &self.error {
            bail!("{error}");
        }
        let temporary = self.path.with_extension("tmp");
        fs::write(&temporary, serde_json::to_vec(&saved)?).context("Cannot save history")?;
        fs::rename(&temporary, &self.path).context("Cannot replace history file")?;
        self.saved = saved;
        self.data_revision += 1;
        Ok(())
    }

    pub fn cancel(&mut self) {
        if let Some(job) = self.job.take() {
            job.store(true, Ordering::Relaxed);
        }
        self.generation += 1;
        self.progress = "Cancelled; cached games remain available".into();
    }

    pub fn account(&mut self, provider: Provider, username: Option<String>) -> anyhow::Result<()> {
        let username = username.map(|s| s.trim().to_ascii_lowercase());
        if let Some(name) = &username {
            if name.len() < 2
                || name.len() > 25
                || !name
                    .bytes()
                    .all(|b| b.is_ascii_alphanumeric() || b == b'_' || b == b'-')
            {
                bail!("Use a username of 2–25 letters, numbers, underscores or hyphens");
            }
            if self
                .saved
                .accounts
                .iter()
                .any(|a| a.provider == provider && a.username == *name)
            {
                return Ok(());
            }
        }
        let mut next = self.saved.clone();
        next.accounts.retain(|a| a.provider != provider);
        if let Some(username) = username {
            next.accounts.push(Account {
                provider,
                username,
                last_success_ms: None,
                attempted_ms: None,
                error: None,
                coverage: Coverage::default(),
                games: vec![],
            });
        }
        self.persist(next)?;
        self.cancel();
        self.progress.clear();
        Ok(())
    }

    pub fn merge(
        &mut self,
        generation: u64,
        account: &Account,
        games: Vec<Game>,
        coverage: Coverage,
        error: Option<String>,
    ) -> anyhow::Result<bool> {
        if self.generation != generation {
            return Ok(false);
        }
        let mut saved = self.saved.clone();
        let target = saved
            .accounts
            .iter_mut()
            .find(|a| a.provider == account.provider && a.username == account.username)
            .context("Account changed during refresh")?;
        for game in games {
            target.games.retain(|old| old.id != game.id);
            target.games.push(game);
        }
        target
            .games
            .retain(|g| g.played_ms >= coverage.since_ms && g.played_ms <= coverage.until_ms);
        target
            .games
            .sort_by(|a, b| b.played_ms.cmp(&a.played_ms).then(a.id.cmp(&b.id)));
        target.games.truncate(GAME_LIMIT);
        target.attempted_ms = Some(coverage.until_ms);
        if error.is_none() {
            target.last_success_ms = Some(coverage.until_ms);
        }
        target.error = error;
        target.coverage = coverage;
        self.persist(saved)?;
        Ok(true)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    struct Directory(PathBuf);
    impl Directory {
        fn new() -> Self {
            let path = std::env::temp_dir().join(format!(
                "history-{}-{}",
                std::process::id(),
                fastrand::u64(..)
            ));
            fs::create_dir(&path).expect("Create test directory");
            Self(path)
        }
    }
    impl Drop for Directory {
        fn drop(&mut self) {
            fs::remove_dir_all(&self.0).expect("Remove test directory");
        }
    }
    fn game(account: &Account, id: &str, time: i64) -> Game {
        Game {
            provider: account.provider,
            account: account.username.clone(),
            id: id.into(),
            url: format!("https://lichess.org/{id}"),
            started_ms: Some(time),
            played_ms: time,
            white: account.username.clone(),
            black: "opponent".into(),
            color: "white".into(),
            result: "1-0".into(),
            time_control: "blitz".into(),
            pgn: "1. e4 1-0".into(),
            moves: crate::history_review::parse("1. e4 1-0").expect("Legal fixture"),
        }
    }
    fn coverage() -> Coverage {
        Coverage {
            since_ms: 1000,
            until_ms: 2000,
            ..Coverage::default()
        }
    }

    /// Failed disk replacement must preserve memory and the prior on-disk
    /// account. Successful replacement can change identity and clears history.
    #[test]
    fn persistence_is_transactional_and_removal_survives_restart() {
        let dir = Directory::new();
        let mut store = Store::load(dir.0.clone());
        store
            .account(Provider::Lichess, Some("Alice".into()))
            .expect("Save account offline");
        fs::create_dir(store.path.with_extension("tmp")).expect("Block temp-file write");
        assert!(store
            .account(Provider::Lichess, Some("bob".into()))
            .is_err());
        assert_eq!(store.saved.accounts[0].username, "alice");
        assert_eq!(
            Store::load(dir.0.clone()).saved.accounts[0].username,
            "alice"
        );
        fs::remove_dir(store.path.with_extension("tmp")).expect("Unblock writes");
        store
            .account(Provider::Lichess, None)
            .expect("Remove account");
        assert!(Store::load(dir.0.clone()).saved.accounts.is_empty());
    }

    /// Unknown schemas and corrupt files must be surfaced without being replaced.
    /// A supported version must still load after an ordinary restart.
    #[test]
    fn preserves_unreadable_files() {
        let dir = Directory::new();
        for bytes in ["{broken", r#"{"version":99,"accounts":[]}"#] {
            fs::write(dir.0.join("history.json"), bytes).expect("Write fixture");
            let mut store = Store::load(dir.0.clone());
            assert!(store.error.is_some());
            assert!(store
                .account(Provider::ChessCom, Some("alice".into()))
                .is_err());
            assert_eq!(
                fs::read_to_string(dir.0.join("history.json")).expect("Read preserved file"),
                bytes
            );
        }
    }

    /// Refreshes upsert IDs, not move sequences, and preserve prior games on
    /// partial failure. A later successful rescan can advance last-success time.
    #[test]
    fn partial_refresh_deduplicates_and_preserves_success() {
        let dir = Directory::new();
        let mut store = Store::load(dir.0.clone());
        store
            .account(Provider::Lichess, Some("alice".into()))
            .expect("Account");
        let account = store.saved.accounts[0].clone();
        let generation = store.generation;
        store
            .merge(
                generation,
                &account,
                vec![game(&account, "aaaaaaaa", 1100)],
                coverage(),
                None,
            )
            .expect("Initial fetch");
        store
            .merge(
                generation,
                &account,
                vec![
                    game(&account, "bbbbbbbb", 1200),
                    game(&account, "bbbbbbbb", 1200),
                ],
                Coverage {
                    until_ms: 2100,
                    ..coverage()
                },
                Some("Offline".into()),
            )
            .expect("Partial fetch");
        let current = &store.saved.accounts[0];
        assert_eq!(current.games.len(), 2);
        assert_eq!(current.last_success_ms, Some(2000));
        assert_eq!(current.attempted_ms, Some(2100));
        store
            .merge(
                generation,
                &account,
                vec![game(&account, "aaaaaaaa", 1100)],
                Coverage {
                    since_ms: 1150,
                    until_ms: 2200,
                    ..coverage()
                },
                None,
            )
            .expect("Rescan window");
        assert_eq!(store.saved.accounts[0].games.len(), 1);
        assert_eq!(
            Store::load(dir.0.clone()).saved.accounts[0].last_success_ms,
            Some(2200)
        );
    }

    /// Late responses cannot undo cancellation, replacement or removal. A new
    /// job for the new identity remains allowed to commit.
    #[test]
    fn stale_jobs_cannot_resurrect_accounts() {
        let dir = Directory::new();
        let mut store = Store::load(dir.0.clone());
        store
            .account(Provider::Lichess, Some("alice".into()))
            .expect("Account");
        for change in [0, 1, 2] {
            let account = store.saved.accounts[0].clone();
            let generation = store.generation;
            let cancel = Arc::new(AtomicBool::new(false));
            store.job = Some(cancel.clone());
            match change {
                0 => store.cancel(),
                1 => store
                    .account(Provider::Lichess, Some("bob".into()))
                    .expect("Replace account"),
                _ => store
                    .account(Provider::Lichess, None)
                    .expect("Remove account"),
            }
            assert!(cancel.load(Ordering::Relaxed));
            assert!(!store
                .merge(
                    generation,
                    &account,
                    vec![game(&account, "aaaaaaaa", 1200)],
                    coverage(),
                    None
                )
                .expect("Reject stale result"));
            assert!(store.saved.accounts.iter().all(|a| a.games.is_empty()));
        }
    }

    /// Tied timestamps use stable IDs; changing insertion order must not change
    /// retention. Old records expire at the explicit window boundary.
    #[test]
    fn bounds_cache_and_orders_ties() {
        let dir = Directory::new();
        let mut store = Store::load(dir.0.clone());
        store
            .account(Provider::Lichess, Some("alice".into()))
            .expect("Account");
        let account = store.saved.accounts[0].clone();
        let games = (0..110)
            .rev()
            .map(|n| game(&account, &format!("{n:08}"), 1000))
            .collect();
        store
            .merge(store.generation, &account, games, coverage(), None)
            .expect("Bound records");
        assert_eq!(store.saved.accounts[0].games.len(), GAME_LIMIT);
        assert_eq!(store.saved.accounts[0].games[0].id, "00000000");
    }
}
