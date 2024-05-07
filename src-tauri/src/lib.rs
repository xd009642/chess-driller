use anyhow::{anyhow, bail};
use serde::{Deserialize, Serialize};
use shakmaty::{san::SanPlus, Chess, Color, Position, Role, Square};
use std::path::PathBuf;
use std::str::FromStr;
use std::sync::Mutex;
use tracing::{error, info};

pub mod clients;
pub mod config;
pub mod db;
pub mod game;

pub use crate::clients::chess_com::*;
pub use crate::config::*;
pub use crate::db::*;

pub struct ChessState(Mutex<App>);

#[derive(Clone)]
pub struct App {
    chess_com_usernames: Vec<String>,
    db: OpeningDatabase,
    color: Color,
    game: Chess,
    game_state: Option<GameState>,
    moves: Vec<SanPlus>,
}

fn create_app() -> anyhow::Result<App> {
    let config = Config::load()?;
    let chess_dot_com = ChessComClient::new();
    let db = OpeningDatabase::load_default()?;

    let game_state = db.start_drill(Color::White, &[]);

    Ok(App {
        chess_com_usernames: config.chess_com,
        db,
        color: Color::White,
        game: Chess::new(),
        moves: vec![],
        game_state,
    })
}

pub fn launch() {
    tauri::Builder::default()
        .manage(ChessState(Mutex::new(create_app().unwrap())))
        .invoke_handler(tauri::generate_handler![
            commands::move_piece,
            commands::start,
            commands::reset
        ])
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}

pub mod commands {
    use super::*;
    use tauri::State;

    #[tauri::command]
    pub fn start(state: State<ChessState>) -> String {
        let mut state = state.0.lock().unwrap();
        if state.color == Color::White {
            state.game_state = state.db.start_drill(Color::White, &state.moves);
        } else {
            state.game_state = state.db.start_drill(Color::Black, &state.moves);
        }
        let mut game_state = state.game_state.take();
        if let Some(game_state) = game_state.as_mut() {
            if !game_state.is_player_turn() {
                let mv = game_state.make_move(state.db.graph(state.color));
                if let Some(mv) = mv {
                    let game = state.game.clone();

                    let mv = mv.san.to_move(&game).unwrap();
                    let new_game = game.clone().play(&mv).unwrap();
                    state.game = new_game;
                }
            }
        }
        state.game_state = game_state;
        state.game.board().to_string()
    }

    #[tauri::command]
    pub fn reset(color: &str, state: State<ChessState>) {
        info!("Resetting board for {}", color);
        let mut state = state.0.lock().unwrap();
        let color = Color::from_str(color).unwrap();
        state.color = color;
        state.game = Chess::new();
        state.game_state = None;
        state.moves.clear();
        info!("Board reset");
    }

    #[tauri::command]
    pub fn move_piece(from: &str, to: &str, promotion: &str, state: State<ChessState>) -> String {
        info!("Args: {}->{} {}", from, to, promotion);

        let mut state = state.0.lock().unwrap();
        let sel_square = Square::from_ascii(from.as_bytes()).unwrap();
        let promotion_square = Square::from_ascii(to.as_bytes()).unwrap();

        let board = state.game.board();

        let piece = board.piece_at(sel_square).unwrap();

        let moves = state.game.san_candidates(piece.role, promotion_square);

        // Move wasn't legal!
        if moves.is_empty() {
            return state.game.board().to_string();
        }

        // There must be a promotion available!
        let game_move = if moves.len() > 1 {
            let promo = Role::from_char(promotion.chars().nth(1).unwrap()).unwrap();
            moves.iter().find(|x| x.promotion() == Some(promo)).unwrap()
        } else {
            &moves[0]
        };

        info!("Move list: {:?}", moves);

        let san = SanPlus::from_move(state.game.clone(), game_move);

        let game = state.game.clone();
        match game.play(game_move) {
            Ok(new_game) => {
                state.game = new_game;
                let mut game_state = state.game_state.take();
                let graph = state.db.graph(state.color);
                if let Some(game_state) = game_state.as_mut() {
                    let prep_state = game_state.apply_move(&san, graph);
                    info!("Prep status: {:?}", prep_state);
                    if let Some(mv) = game_state.make_move(graph) {
                        let game = state.game.clone();

                        let mv = mv.san.to_move(&game).unwrap();
                        let new_game = game.clone().play(&mv).unwrap();
                        state.game = new_game;
                    }
                } else {
                    state.moves.push(san);
                }
                state.game_state = game_state;
            }
            Err(e) => {
                error!("{}", e);
            }
        }
        state.game.board().to_string()
    }
}
/*
pub fn run() -> anyhow::Result<()> {
    let config = Config::load()?;
    let chess_dot_com = ChessComClient::new();
    let _user_games = chess_dot_com.download_all_games(&config);
    let database = OpeningDatabase::load_default()?;

    let mut running = true;

    let mut board = Board::default();
    // Just putting here to decide if we want to store the openings as a graph of `Board` because
    // that might be fast and simple :thinking:
    info!("Board is: {} bytes in memory", std::mem::size_of::<Board>());

    // Without changing the graph structure we need to start tracking the moves from the very
    // beginning for both white and black - so we'll have a node-index into both.

    let mut selected_square = None;
    let mut san_moves = vec![];
    let mut game_state: Option<GameState> = None;
    let mut drag_context = None;
    let mut pending_promotion_square = None;
    let mut promotion_from = None;
    while running {
        window.render(
            &board,
            selected_square,
            drag_context,
            promotion_from.zip(pending_promotion_square),
        );

        let pending_events = events.handle_events();

        for event in &pending_events {
            match event.kind {
                EventKind::Close => {
                    info!("Closing");
                    running = false;
                }
                EventKind::FlipBoard => {
                    window.flip();
                }
                EventKind::Reset => {
                    san_moves.clear();
                    game_state = None;
                    board = Board::default();
                }
                EventKind::MouseClick { x, y } => {
                    if let Some((promotion_square, sel_square)) =
                        pending_promotion_square.zip(promotion_from)
                    {
                        let promotion_file = promotion_square.get_file().to_index() as i32;
                        let promotion_rank = promotion_square.get_rank().to_index() as i32;

                        if let Some(square) = window.get_square(x, y) {
                            let square_file = square.get_file().to_index() as i32;
                            let square_rank = square.get_rank().to_index() as i32;
                            let file_offset = if promotion_file == 7 { -1 } else { 1 };
                            let rank_offset = if promotion_rank == 0 { -1 } else { 1 };
                            let mut piece = chess::Piece::Queen;

                            if square_file == promotion_file + file_offset {
                                if square_rank == promotion_rank {
                                    piece = chess::Piece::Queen;
                                } else if square_rank == promotion_rank - rank_offset {
                                    piece = chess::Piece::Rook;
                                } else if square_rank == promotion_rank - rank_offset * 2 {
                                    piece = chess::Piece::Bishop;
                                } else if square_rank == promotion_rank - rank_offset * 3 {
                                    piece = chess::Piece::Knight;
                                }

                                let candidate_move =
                                    ChessMove::new(sel_square, promotion_square, Some(piece));

                                if board.legal(candidate_move) {
                                    board = board.make_move_new(candidate_move);
                                }
                            }
                        }

                        selected_square = None;
                        pending_promotion_square = None;
                        promotion_from = None;
                    } else if let Some(square) = window.get_square(x, y) {
                        if let Some(s) = selected_square {
                            let candidate_move = ChessMove::new(s, square, None);
                            if board.legal(candidate_move) {
                                if let Some(san) = game::get_san(candidate_move, &board) {
                                    info!("{}", san);
                                    board = board.make_move_new(candidate_move);
                                    if let Some(state) = game_state.as_mut() {
                                        let prep_status = state.apply_move(&san);
                                        if prep_status == MoveAssessment::InPrep {
                                            if let Some(mv) = state.make_move() {
                                                let text = mv.to_string();
                                                info!("{}", text);
                                                board = board.make_move_new(
                                                    ChessMove::from_san(&board, &text).unwrap(),
                                                );
                                            }
                                        } else {
                                            info!("You've hit the end: {:?}", prep_status);
                                        }
                                    } else {
                                        san_moves.push(san);
                                    }
                                } else {
                                    info!("Something went wrong didn't record this move");
                                }
                                selected_square = None;
                            } else {
                                selected_square = Some(square);
                            }
                        } else if board.piece_on(square).is_some() {
                            selected_square = Some(square);
                        }
                    }
                }
                EventKind::StartPractising => {
                    if let Some(state) = game_state.as_ref() {
                        if state.still_running() {
                            continue;
                        }
                        board = Board::default();
                    }
                    game_state = None;
                    info!("Lets start playing!");
                    game_state = database.start_drill(window.player(), &san_moves);
                    if let Some(state) = game_state.as_mut() {
                        if !state.is_player_turn() {
                            info!("Not the players turn, lets make a move");
                            if let Some(mv) = state.make_move() {
                                info!("I made a move?");
                                let text = mv.to_string();
                                info!("{}", text);
                                board = board
                                    .make_move_new(ChessMove::from_san(&board, &text).unwrap());
                            }
                        }
                    }
                }
                // TODO: long click mouse up mouse down?
                _ => {}
            }
        }
    }

    std::mem::drop(window);

    Ok(())
}
*/
