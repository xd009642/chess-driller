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
const SELECTED_LIGHT_SQUARE: Color = Color::RGB(0xF6, 0xEA, 0x71);
const SELECTED_DARK_SQUARE: Color = Color::RGB(0xDB, 0xC3, 0x4A);

#[derive(Debug, Clone, Copy)]
pub struct DragContext {
    pub current_x: i32,
    pub current_y: i32,
}

pub struct RenderSystem<'s> {
    flipped: bool,
    square_size: u32,
    canvas: &'s mut Canvas<Window>,
    width: u32,
    main_texture: Texture<'s>,
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

        let main_texture = texture_creator
            .create_texture_target(canvas.default_pixel_format(), width, width)
            .unwrap();
        Ok(Self {
            flipped,
            square_size: width / 8,
            canvas,
            width,
            main_texture,
            sprites,
        })
    }

    /// (╯°□°)╯︵ ┻━┻.
    pub fn flip(&mut self) {
        self.flipped = !self.flipped;
    }

    /// Return what colour we're currently playing as
    pub fn player(&self) -> SquareColor {
        if self.flipped {
            SquareColor::Black
        } else {
            SquareColor::White
        }
    }

    pub fn render(
        &mut self,
        board: &Board,
        selected_square: Option<Square>,
        drag_context: Option<DragContext>,
        promotion_from_to: Option<(Square, Square)>,
    ) {
        self.draw_board(selected_square);
        self.draw_pieces(
            board,
            selected_square,
            drag_context,
            promotion_from_to.map(|(_, to)| to),
        );

        if let Some((from, to)) = promotion_from_to {
            self.render_promotion_picker(
                to,
                board.color_on(from).expect("no piece on promotion square?"),
            );
        }

        self.canvas
            .copy(
                &self.main_texture,
                None,
                Rect::new(0, 0, self.width, self.width),
            )
            .unwrap();

        self.canvas.present();
    }

    fn draw_pieces(
        &mut self,
        board: &Board,
        selected_square: Option<Square>,
        drag_context: Option<DragContext>,
        active_promotion: Option<Square>,
    ) {
        self.canvas
            .with_texture_canvas(&mut self.main_texture, |canvas| {
                for i in 0..64 {
                    let square = unsafe { Square::new(i) };
                    if let Some(piece) = board.piece_on(square) {
                        if (selected_square == Some(square) && drag_context.is_some())
                            || active_promotion == Some(square)
                        {
                            continue;
                        }

                        let color = board.color_on(square).unwrap();
                        let (rank, file) = Self::rank_and_file(self.flipped, square);
                        let sprite = &self.sprites[&(piece, color)];

                        canvas
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

                if let Some((active_promotion, square)) = active_promotion.zip(selected_square) {
                    if let Some(piece) = board.piece_on(square) {
                        let color = board.color_on(square).unwrap();
                        let (rank, file) = Self::rank_and_file(self.flipped, active_promotion);
                        let sprite = &self.sprites[&(piece, color)];

                        canvas
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
                } else if let Some((square, drag_context)) = selected_square.zip(drag_context) {
                    if let Some(piece) = board.piece_on(square) {
                        let color = board.color_on(square).unwrap();
                        let sprite = &self.sprites[&(piece, color)];

                        canvas
                            .copy(
                                sprite,
                                None,
                                Rect::new(
                                    drag_context.current_x - (self.square_size / 2) as i32,
                                    drag_context.current_y - (self.square_size / 2) as i32,
                                    self.square_size,
                                    self.square_size,
                                ),
                            )
                            .unwrap();
                    }
                }
            })
            .unwrap();
    }

    fn draw_board(&mut self, selected_square: Option<Square>) {
        self.canvas
            .with_texture_canvas(&mut self.main_texture, |canvas| {
                if self.flipped {
                    canvas.set_draw_color(LIGHT_SQUARE);
                } else {
                    canvas.set_draw_color(DARK_SQUARE);
                }

                canvas.clear();

                if self.flipped {
                    canvas.set_draw_color(DARK_SQUARE);
                } else {
                    canvas.set_draw_color(LIGHT_SQUARE);
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
                        let _ = canvas.fill_rect(rect);
                    }
                    row += 1;
                }

                if let Some(square) = selected_square {
                    let selected_color = match square.get_file().to_index() % 2 == 0 {
                        true => match square.get_rank().to_index() % 2 == 0 {
                            true => SELECTED_DARK_SQUARE,
                            false => SELECTED_LIGHT_SQUARE,
                        },
                        false => match square.get_rank().to_index() % 2 == 0 {
                            true => SELECTED_LIGHT_SQUARE,
                            false => SELECTED_DARK_SQUARE,
                        },
                    };

                    canvas.set_draw_color(selected_color);

                    let (rank, file) = Self::rank_and_file(self.flipped, square);
                    let rect = Rect::new(
                        (file * self.square_size) as i32,
                        (rank * self.square_size) as i32,
                        self.square_size,
                        self.square_size,
                    );

                    let _ = canvas.fill_rect(rect);
                }
            })
            .unwrap();
    }

    pub fn render_promotion_picker(&mut self, square: Square, color: chess::Color) {
        self.main_texture
            .set_blend_mode(sdl2::render::BlendMode::Blend);
        self.canvas
            .with_texture_canvas(&mut self.main_texture, |canvas| {
                canvas.set_blend_mode(sdl2::render::BlendMode::Blend);
                canvas.set_draw_color(Color::RGBA(0, 0, 0, 0x40));
                canvas
                    .fill_rect(Rect::new(0, 0, self.width, self.width))
                    .unwrap();
                canvas.set_draw_color(Color::RGB(0xFF, 0xFF, 0xFF));

                let (rank, file) = Self::rank_and_file(self.flipped, square);
                let offset = if file == 7 { -1i32 } else { 1 };

                for (i, piece) in [Piece::Queen, Piece::Rook, Piece::Bishop, Piece::Knight]
                    .into_iter()
                    .enumerate()
                {
                    let i = match (self.flipped, rank) {
                        (true, 0) => i as i32,
                        (false, 7) => -(i as i32),
                        (false, 0) => i as i32,
                        (true, 7) => -(i as i32),
                        _ => unreachable!(),
                    };

                    let rect = Rect::new(
                        (file as i32 + offset) * self.square_size as i32,
                        (rank as i32 + i) * self.square_size as i32,
                        self.square_size,
                        self.square_size,
                    );

                    let _ = canvas.fill_rect(rect);

                    canvas
                        .copy(
                            self.sprites.get(&(piece, color)).unwrap(),
                            None,
                            Rect::new(
                                (file as i32 + offset) * self.square_size as i32,
                                (rank as i32 + i) * self.square_size as i32,
                                self.square_size,
                                self.square_size,
                            ),
                        )
                        .unwrap();
                }
            })
            .unwrap();
    }

    pub fn get_square(&self, x: i32, y: i32) -> Option<Square> {
        let mut norm_x = (x as f32 / self.square_size as f32).floor();
        let mut norm_y = (y as f32 / self.square_size as f32).floor();

        if !self.flipped {
            norm_y = 7.0 - norm_y;
        } else {
            norm_x = 7.0 - norm_x;
        }

        if norm_x < 0.0 || norm_x > 7.0 || norm_y < 0.0 || norm_y > 7.0 {
            return None;
        }

        let rank = Rank::from_index(norm_y as usize);
        let file = File::from_index(norm_x as usize);

        Some(Square::make_square(rank, file))
    }

    // FIXME: this can't be a method because of the `.with_texture_canvas` calls
    fn rank_and_file(flipped: bool, square: Square) -> (u32, u32) {
        let mut rank = square.get_rank().to_index() as u32;
        let mut file = square.get_file().to_index() as u32;
        if !flipped {
            rank = 7 - rank;
        } else {
            file = 7 - file;
        }

        (rank, file)
    }
}
