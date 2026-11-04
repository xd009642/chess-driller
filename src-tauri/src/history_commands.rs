use crate::{
    history::{self, Provider},
    history_providers, history_review,
    repertoire::Repertoire,
    App,
};
use serde::Serialize;
use std::sync::{atomic::AtomicBool, Arc, Mutex};
use tauri::State;

pub struct HistoryState(pub Arc<Mutex<history::Store>>);
fn lock(state: &HistoryState) -> Result<std::sync::MutexGuard<'_, history::Store>, String> {
    state
        .0
        .lock()
        .map_err(|_| "History state unavailable; restart the app".into())
}

#[tauri::command]
pub fn history_state(state: State<'_, HistoryState>) -> Result<history::View, String> {
    Ok(lock(&state)?.view())
}

#[tauri::command]
pub fn save_account(
    provider: Provider,
    username: Option<String>,
    state: State<'_, HistoryState>,
) -> Result<history::View, String> {
    let mut store = lock(&state)?;
    store
        .account(provider, username)
        .map_err(|e| format!("{e:#}"))?;
    Ok(store.view())
}

#[tauri::command]
pub fn cancel_history(state: State<'_, HistoryState>) -> Result<history::View, String> {
    let mut store = lock(&state)?;
    store.cancel();
    Ok(store.view())
}

#[tauri::command]
pub async fn refresh_history(state: State<'_, HistoryState>) -> Result<(), String> {
    let (accounts, generation, cancel) = {
        let mut store = lock(&state)?;
        if store.job.is_some() {
            return Err("Refresh is already running".into());
        }
        if let Some(error) = &store.error {
            return Err(error.clone());
        }
        if store.saved.accounts.is_empty() {
            return Err("Save an account in Settings first".into());
        }
        let cancel = Arc::new(AtomicBool::new(false));
        store.job = Some(cancel.clone());
        store.generation += 1;
        store.progress = "Starting refresh…".into();
        (store.saved.accounts.clone(), store.generation, cancel)
    };
    let shared = state.0.clone();
    let result = tauri::async_runtime::spawn_blocking(move || -> Result<(), String> {
        for account in accounts {
            if cancel.load(std::sync::atomic::Ordering::Relaxed) {
                break;
            }
            let downloaded = history_providers::fetch(&account, &cancel, |message| {
                if let Ok(mut store) = shared.lock() {
                    if store.generation == generation {
                        store.progress = message;
                    }
                }
            });
            let mut store = shared.lock().map_err(|_| "History state unavailable")?;
            if !store
                .merge(
                    generation,
                    &account,
                    downloaded.games,
                    downloaded.coverage,
                    downloaded.error,
                )
                .map_err(|e| format!("{e:#}"))?
            {
                break;
            }
        }
        Ok(())
    })
    .await
    .map_err(|e| e.to_string())
    .and_then(|r| r);
    let mut store = lock(&state)?;
    if store.generation == generation {
        store.job = None;
        store.progress = match &result {
            Ok(()) => "Refresh finished; see each account’s coverage below".into(),
            Err(e) => e.clone(),
        };
    }
    result
}

#[derive(Serialize)]
pub struct Reviewed {
    pub game: history::Game,
    pub review: history_review::Review,
}

#[tauri::command]
pub async fn review_history(
    selected: Vec<u64>,
    state: State<'_, HistoryState>,
    app: State<'_, Mutex<App>>,
) -> Result<Vec<Reviewed>, String> {
    let (revision, games) = {
        let store = lock(&state)?;
        (
            store.data_revision,
            store
                .saved
                .accounts
                .iter()
                .flat_map(|a| a.games.clone())
                .collect::<Vec<_>>(),
        )
    };
    let files = app
        .lock()
        .map_err(|_| "Application state unavailable")?
        .library
        .files
        .clone();
    let file_ids: Vec<_> = files.iter().map(|f| f.id).collect();
    if selected.iter().any(|id| !file_ids.contains(id)) {
        return Err("A selected prep file no longer exists".into());
    }
    let result = tauri::async_runtime::spawn_blocking(move || -> anyhow::Result<Vec<Reviewed>> {
        let build = |color: &str| {
            let chosen: Vec<_> = files
                .iter()
                .filter(|f| f.color == color && selected.contains(&f.id))
                .cloned()
                .collect();
            if chosen.is_empty() {
                Ok(None)
            } else {
                Repertoire::from_files(&chosen).map(Some)
            }
        };
        let white = build("white")?;
        let black = build("black")?;
        let mut rows = Vec::new();
        for mut game in games {
            // Validate persisted PGN too: disk contents are not a trusted move oracle.
            game.moves = history_review::parse(&game.pgn)?;
            history_providers::game_url(game.provider, &game.url)?;
            let color = if game.color == "white" {
                shakmaty::Color::White
            } else {
                shakmaty::Color::Black
            };
            let review = history_review::compare(
                &game.moves,
                color,
                if color == shakmaty::Color::White {
                    white.as_ref()
                } else {
                    black.as_ref()
                },
            )?;
            rows.push(Reviewed { game, review });
        }
        rows.sort_by(|a, b| {
            b.game
                .played_ms
                .cmp(&a.game.played_ms)
                .then(a.game.id.cmp(&b.game.id))
        });
        Ok(rows)
    })
    .await
    .map_err(|e| e.to_string())?
    .map_err(|e| format!("{e:#}"))?;
    if lock(&state)?.data_revision != revision {
        return Err("History changed; review again".into());
    }
    let current_ids: Vec<_> = app
        .lock()
        .map_err(|_| "Application state unavailable")?
        .library
        .files
        .iter()
        .map(|f| f.id)
        .collect();
    if current_ids != file_ids {
        return Err("Prep changed; review again".into());
    }
    Ok(result)
}

#[tauri::command]
pub fn open_history_game(provider: Provider, url: String) -> Result<(), String> {
    history_providers::game_url(provider, &url).map_err(|e| e.to_string())?;
    #[cfg(target_os = "linux")]
    let mut command = std::process::Command::new("xdg-open");
    #[cfg(target_os = "macos")]
    let mut command = std::process::Command::new("open");
    #[cfg(target_os = "windows")]
    let mut command = {
        let mut cmd = std::process::Command::new("rundll32.exe");
        cmd.arg("url.dll,FileProtocolHandler");
        cmd
    };
    command
        .arg(url)
        .spawn()
        .map_err(|e| format!("Cannot open browser: {e}"))?;
    Ok(())
}
