use chess::{Board, Color as SquareColor, File, Piece, Rank, Square};

use sdl2::image::LoadTexture;
use sdl2::pixels::Color;
use sdl2::rect::Rect;
use sdl2::render::{Canvas, Texture, TextureCreator};
use sdl2::video::{Window, WindowContext};

use std::collections::HashMap;
use std::path::Path;

const LIGHT_SQUARE: Color = Color::RGB(0xFF, 0xCE, 0x9E);
const DARK_SQUARE: Color = Color::RGB(0xD1, 0x8B, 0x47);

pub struct RenderSystem<'s> {
    flipped: bool,
    square_size: u32,
    canvas: &'s mut Canvas<Window>,

    sprites: HashMap<(Piece, SquareColor), Texture<'s>>,
}

impl<'s> RenderSystem<'s> {
    pub fn new(
        flipped: bool,
        width: u32,
        canvas: &'s mut Canvas<Window>,
        texture_creator: &'s TextureCreator<WindowContext>,
    ) -> anyhow::Result<Self> {
        let mut sprites = HashMap::new();
        sprites.insert(
            (Piece::Pawn, SquareColor::White),
            texture_creator
                .load_texture(Path::new("resources/p_white.png"))
                .unwrap(),
        );
        sprites.insert(
            (Piece::Knight, SquareColor::White),
            texture_creator
                .load_texture(Path::new("resources/n_white.png"))
                .unwrap(),
        );
        sprites.insert(
            (Piece::Bishop, SquareColor::White),
            texture_creator
                .load_texture(Path::new("resources/b_white.png"))
                .unwrap(),
        );
        sprites.insert(
            (Piece::Rook, SquareColor::White),
            texture_creator
                .load_texture(Path::new("resources/r_white.png"))
                .unwrap(),
        );
        sprites.insert(
            (Piece::Queen, SquareColor::White),
            texture_creator
                .load_texture(Path::new("resources/q_white.png"))
                .unwrap(),
        );
        sprites.insert(
            (Piece::King, SquareColor::White),
            texture_creator
                .load_texture(Path::new("resources/k_white.png"))
                .unwrap(),
        );
        sprites.insert(
            (Piece::Pawn, SquareColor::Black),
            texture_creator
                .load_texture(Path::new("resources/p_black.png"))
                .unwrap(),
        );
        sprites.insert(
            (Piece::Knight, SquareColor::Black),
            texture_creator
                .load_texture(Path::new("resources/n_black.png"))
                .unwrap(),
        );
        sprites.insert(
            (Piece::Bishop, SquareColor::Black),
            texture_creator
                .load_texture(Path::new("resources/b_black.png"))
                .unwrap(),
        );
        sprites.insert(
            (Piece::Rook, SquareColor::Black),
            texture_creator
                .load_texture(Path::new("resources/r_black.png"))
                .unwrap(),
        );
        sprites.insert(
            (Piece::Queen, SquareColor::Black),
            texture_creator
                .load_texture(Path::new("resources/q_black.png"))
                .unwrap(),
        );
        sprites.insert(
            (Piece::King, SquareColor::Black),
            texture_creator
                .load_texture(Path::new("resources/k_black.png"))
                .unwrap(),
        );
        Ok(Self {
            flipped,
            square_size: width / 8,
            canvas,
            sprites,
        })
    }

    /// (╯°□°)╯︵ ┻━┻.
    pub fn flip(&mut self) {
        self.flipped = !self.flipped;
    }

    pub fn render(&mut self, board: &Board) {
        self.draw_board();
        self.draw_pieces(board);

        self.canvas.present();
    }

    fn draw_pieces(&mut self, board: &Board) {
        for i in 0..64 {
            let square = unsafe { Square::new(i) };
            if let Some(piece) = board.piece_on(square.clone()) {
                let color = board.color_on(square.clone()).unwrap();

                let mut rank = square.get_rank().to_index() as u32;
                if !self.flipped {
                    rank = 7 - rank;
                }
                let file = square.get_file().to_index() as u32;

                let sprite = &self.sprites[&(piece, color)];

                self.canvas
                    .copy(
                        sprite,
                        None,
                        Rect::new(
                            (file * self.square_size) as i32,
                            (rank * self.square_size) as i32,
                            self.square_size,
                            self.square_size,
                        ),
                    )
                    .unwrap();
            }
        }
    }

    fn draw_board(&mut self) {
        if self.flipped {
            self.canvas.set_draw_color(LIGHT_SQUARE);
        } else {
            self.canvas.set_draw_color(DARK_SQUARE);
        }
        self.canvas.clear();

        if self.flipped {
            self.canvas.set_draw_color(DARK_SQUARE);
        } else {
            self.canvas.set_draw_color(LIGHT_SQUARE);
        }

        let mut row = 0;
        while row < 8 {
            let mut x = row % 2;
            for _ in (row % 2)..(4 + (row % 2)) {
                let rect = Rect::new(
                    x * self.square_size as i32,
                    row * self.square_size as i32,
                    self.square_size,
                    self.square_size,
                );
                x += 2;
                let _ = self.canvas.fill_rect(rect);
            }
            row += 1;
        }
    }

    pub fn get_square(&self, x: i32, y: i32) -> Option<Square> {
        let norm_x = (x as f32 / self.square_size as f32).floor();
        let mut norm_y = (y as f32 / self.square_size as f32).floor();

        if !self.flipped {
            norm_y = 7.0 - norm_y;
        }

        if norm_x < 0.0 || norm_x > 7.0 || norm_y < 0.0 || norm_y > 7.0 {
            return None;
        }

        let rank = Rank::from_index(norm_y as usize);
        let file = File::from_index(norm_x as usize);

        Some(Square::make_square(rank, file))
    }
}
