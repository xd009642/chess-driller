use crate::prelude::*;
use anyhow::{anyhow, bail};
use chess::Board;
use sdl2::image::Sdl2ImageContext;
use sdl2::image::{InitFlag, LoadTexture};
use sdl2::pixels::Color;
use sdl2::rect::Rect;
use sdl2::render::{Canvas, Texture, TextureCreator};
use sdl2::video::{Window, WindowContext};
use sdl2::Sdl;

pub mod db;
pub mod events;
pub mod render;

pub mod prelude {
    pub use crate::db::*;
    pub use crate::events::*;
    pub use crate::render::*;
}

pub fn run() -> anyhow::Result<()> {
    let ctx = sdl2::init().map_err(|e| anyhow!(e))?;
    let width = 600;
    let video = ctx.video().map_err(|e| anyhow!(e))?;

    let image_context = sdl2::image::init(InitFlag::PNG).map_err(|e| anyhow!(e))?;

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
            }
        }
    }

    std::mem::drop(window);

    Ok(())
}
