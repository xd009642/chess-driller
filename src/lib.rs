use crate::prelude::*;
use anyhow::{anyhow, bail};
use chess::{Board, ChessMove};

use sdl2::image::InitFlag;

pub mod db;
pub mod events;
pub mod game;
pub mod render;

pub mod prelude {
    pub use crate::db::*;
    pub use crate::events::*;
    pub use crate::render::*;
}

pub fn run() -> anyhow::Result<()> {
    let _database = OpeningDatabase::load_default()?;
    let ctx = sdl2::init().map_err(|e| anyhow!(e))?;
    let width = 600;
    let video = ctx.video().map_err(|e| anyhow!(e))?;

    let _image_context = sdl2::image::init(InitFlag::PNG).map_err(|e| anyhow!(e))?;

    let window = match video
        .window("Chess-driller", width, width)
        .position_centered()
        .opengl()
        .build()
    {
        Ok(window) => window,
        Err(err) => bail!("failed to create window: {}", err),
    };

    let mut canvas = window.into_canvas().software().build()?;
    let texture_creator = canvas.texture_creator();

    let mut window = RenderSystem::new(false, width, &mut canvas, &texture_creator)?;
    let mut events = EventSystem::new(ctx)?;
    let mut running = true;

    let mut board = Board::default();
    // Just putting here to decide if we want to store the openings as a graph of `Board` because
    // that might be fast and simple :thinking:
    println!("Board is: {} bytes in memory", std::mem::size_of::<Board>());

    // Without changing the graph structure we need to start tracking the moves from the very
    // beginning for both white and black - so we'll have a node-index into both.

    let mut selected_square = None;
    let mut san_moves = vec![];
    while running {
        window.render(&board);
        let pending_events = events.handle_events();

        for event in &pending_events {
            match event {
                Event::Close => {
                    println!("Closing");
                    running = false;
                }
                Event::FlipBoard => {
                    window.flip();
                }
                Event::Reset => {
                    board = Board::default();
                }
                Event::MouseDown { x, y } => {
                    if let Some(square) = window.get_square(*x, *y) {
                        if let Some(s) = selected_square {
                            let candidate_move = ChessMove::new(s, square, None);
                            if board.legal(candidate_move) {
                                if let Some(san) = game::get_san(candidate_move, &board) {
                                    println!("{}", san);
                                    san_moves.push(san);
                                } else {
                                    println!("Something went wrong didn't record this move");
                                }
                                board = board.make_move_new(candidate_move);
                                selected_square = None;
                            } else {
                                selected_square = Some(square);
                            }
                        } else if board.piece_on(square).is_some() {
                            selected_square = Some(square);
                        }
                    }
                }
                Event::StartPractising => {
                    let _player = window.player();
                }
            }
        }
    }

    std::mem::drop(window);

    Ok(())
}
