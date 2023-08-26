use chess::{Board, Color as SquareColor, File, Piece, Rank, Square};
use egui::TextureHandle;
use egui_extras::image::load_svg_bytes;
use std::fs;
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

pub struct BoardWidget {
    flipped: bool,
    sprites: HashMap<(Piece, SquareColor), TextureHandle>,
}

impl BoardWidget {
    pub fn new(ui: &mut egui::Ui) -> anyhow::Result<Self> {

        let mut sprites = HashMap::new();
        let w_p = ui.ctx().load_text("white knight", load_svg_bytes(fs::read("resources/n_white.svg")?).unwrap(), Default::default());
        let w_b = ui.ctx().load_text("white bishop", load_svg_bytes(fs::read("resources/b_white.svg")?).unwrap(), Default::default());
        let w_r = ui.ctx().load_text("white rook", load_svg_bytes(fs::read("resources/r_white.svg")?).unwrap(), Default::default());
        let w_q = ui.ctx().load_text("white queen", load_svg_bytes(fs::read("resources/q_white.svg")?).unwrap(), Default::default());
        let w_k = ui.ctx().load_text("white king", load_svg_bytes(fs::read("resources/k_white.svg")?).unwrap(), Default::default());

        sprites.insert((Piece::Pawn, SquareColor::White), w_p);
        sprites.insert((Piece::Bishop, SquareColor::White), w_b);
        sprites.insert((Piece::Rook, SquareColor::White), w_r);
        sprites.insert((Piece::Queen, SquareColor::White), w_q);
        sprites.insert((Piece::King, SquareColor::White), w_k);
        
        let b_p = ui.ctx().load_text("black knight", load_svg_bytes(fs::read("resources/n_black.svg")?).unwrap(), Default::default());
        let b_b = ui.ctx().load_text("black bishop", load_svg_bytes(fs::read("resources/b_black.svg")?).unwrap(), Default::default());
        let b_r = ui.ctx().load_text("black rook", load_svg_bytes(fs::read("resources/r_black.svg")?).unwrap(), Default::default());
        let b_q = ui.ctx().load_text("black queen", load_svg_bytes(fs::read("resources/q_black.svg")?).unwrap(), Default::default());
        let b_k = ui.ctx().load_text("black king", load_svg_bytes(fs::read("resources/k_black.svg")?).unwrap(), Default::default());
        sprites.insert((Piece::Pawn, SquareColor::Black), b_p);
        sprites.insert((Piece::Bishop, SquareColor::Black), b_b);
        sprites.insert((Piece::Rook, SquareColor::Black), b_r);
        sprites.insert((Piece::Queen, SquareColor::Black), b_q);
        sprites.insert((Piece::King, SquareColor::Black), b_k);
        
        Ok(Self {
            flipped,
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
                canvas.set_draw_color(DARK_SQUARE);
                canvas.clear();
                canvas.set_draw_color(LIGHT_SQUARE);

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
