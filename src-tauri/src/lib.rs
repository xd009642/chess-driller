use std::sync::Mutex;
use tauri::Manager;

pub mod clients;
pub mod config;
pub mod db;
mod engine;
pub mod game;
mod repertoire;

pub use crate::clients::chess_com::*;
pub use crate::config::*;
pub use crate::db::*;

struct App {
    library: repertoire::Library,
    session: Option<game::Session>,
    revision: u64,
}

pub fn launch() -> anyhow::Result<()> {
    tauri::Builder::default()
        .setup(|app| {
            let library = repertoire::Library::load(app.path().app_data_dir()?)?;
            app.manage(Mutex::new(App {
                library,
                session: None,
                revision: 0,
            }));
            Ok(())
        })
        .invoke_handler(tauri::generate_handler![
            commands::list_prep,
            commands::import_prep,
            commands::start,
            commands::move_piece,
            commands::retry,
            commands::continue_engine,
            commands::engine_move,
        ])
        .run(tauri::generate_context!())?;
    Ok(())
}

mod commands {
    use super::*;
    use anyhow::{bail, Context};
    use serde::Serialize;
    use shakmaty::Color;
    use std::sync::MutexGuard;
    use tauri::State;

    #[derive(Serialize)]
    pub struct PrepSummary {
        id: u64,
        name: String,
        color: String,
    }

    fn lock<'a>(state: &'a State<'_, Mutex<App>>) -> Result<MutexGuard<'a, App>, String> {
        state
            .lock()
            .map_err(|_| "Application state is unavailable; restart the app".into())
    }

    fn summaries(app: &App) -> Vec<PrepSummary> {
        app.library
            .files
            .iter()
            .map(|file| PrepSummary {
                id: file.id,
                name: file.name.clone(),
                color: file.color.clone(),
            })
            .collect()
    }

    #[tauri::command]
    pub fn list_prep(state: State<'_, Mutex<App>>) -> Result<Vec<PrepSummary>, String> {
        let app = lock(&state)?;
        Ok(summaries(&app))
    }

    #[tauri::command]
    pub fn import_prep(
        name: String,
        color: String,
        pgn: String,
        state: State<'_, Mutex<App>>,
    ) -> Result<Vec<PrepSummary>, String> {
        let mut app = lock(&state)?;
        app.library
            .import(name, color, pgn)
            .map_err(|e| format!("{e:#}"))?;
        Ok(summaries(&app))
    }

    #[tauri::command]
    pub fn start(
        color: String,
        selected: Vec<u64>,
        state: State<'_, Mutex<App>>,
    ) -> Result<game::View, String> {
        let mut app = lock(&state)?;
        let start = || -> anyhow::Result<game::Session> {
            let player = match color.as_str() {
                "white" => Color::White,
                "black" => Color::Black,
                _ => bail!("Choose white or black"),
            };
            if selected.is_empty() {
                bail!("Select at least one prep file");
            }
            let mut files = Vec::new();
            for id in selected {
                let file = app
                    .library
                    .files
                    .iter()
                    .find(|f| f.id == id && f.color == color)
                    .context("A selected file does not belong to this colour")?;
                files.push(file.clone());
            }
            let tree = repertoire::Repertoire::from_files(&files)?;
            game::Session::new(tree, player)
        };
        let session = start().map_err(|e| format!("{e:#}"))?;
        let view = session.view();
        app.session = Some(session);
        app.revision += 1;
        Ok(view)
    }

    fn change(
        state: State<'_, Mutex<App>>,
        action: impl FnOnce(&mut game::Session) -> anyhow::Result<()>,
    ) -> Result<game::View, String> {
        let mut app = lock(&state)?;
        let session = app.session.as_mut().ok_or("Start a drill first")?;
        action(session).map_err(|e| format!("{e:#}"))?;
        let view = session.view();
        app.revision += 1;
        Ok(view)
    }

    #[tauri::command]
    pub fn move_piece(uci: String, state: State<'_, Mutex<App>>) -> Result<game::View, String> {
        change(state, |session| session.player_move(&uci))
    }

    #[tauri::command]
    pub fn retry(state: State<'_, Mutex<App>>) -> Result<game::View, String> {
        change(state, game::Session::retry)
    }

    #[tauri::command]
    pub fn continue_engine(state: State<'_, Mutex<App>>) -> Result<game::View, String> {
        change(state, game::Session::continue_engine)
    }

    #[tauri::command]
    pub async fn engine_move(
        executable: String,
        state: State<'_, Mutex<App>>,
    ) -> Result<game::View, String> {
        let (revision, history) = {
            let app = lock(&state)?;
            let session = app.session.as_ref().ok_or("Start a drill first")?;
            if !session.view().needs_engine {
                return Err("No engine move is needed".into());
            }
            (app.revision, session.history.clone())
        };
        let mv =
            tauri::async_runtime::spawn_blocking(move || engine::best_move(&executable, &history))
                .await
                .map_err(|e| e.to_string())?
                .map_err(|e| format!("{e:#}"))?;
        let mut app = lock(&state)?;
        if app.revision != revision {
            return Err("The drill changed while Stockfish was thinking".into());
        }
        let session = app.session.as_mut().ok_or("Start a drill first")?;
        session.engine_move(&mv).map_err(|e| format!("{e:#}"))?;
        let view = session.view();
        app.revision += 1;
        Ok(view)
    }
}
