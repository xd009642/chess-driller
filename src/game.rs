use chess::{Board, ChessMove, Piece};
use pgn_reader::SanPlus;
use shakmaty::{fen::Fen, CastlingMode, Chess, Move, Role};

fn convert_piece(piece: Piece) -> Role {
    match piece {
        Piece::Pawn => Role::Pawn,
        Piece::Knight => Role::Knight,
        Piece::Bishop => Role::Bishop,
        Piece::Rook => Role::Rook,
        Piece::Queen => Role::Queen,
        Piece::King => Role::King,
    }
}

pub fn get_san(mv: ChessMove, board: &Board) -> Option<SanPlus> {
    let board_str = board.to_string();
    let mut fen: Fen = board_str.parse().unwrap();
    if let Some(ep) = fen.0.ep_square.as_mut() {
        // https://github.com/jordanbray/chess/issues/83
        if ep.rank() == shakmaty::Rank::Fifth {
            *ep = shakmaty::Square::from_coords(ep.file(), shakmaty::Rank::Sixth);
        } else if ep.rank() == shakmaty::Rank::Fourth {
            *ep = shakmaty::Square::from_coords(ep.file(), shakmaty::Rank::Third);
        }
    }
    let pos: Chess = fen.into_position(CastlingMode::Standard).unwrap();

    let role = convert_piece(board.piece_on(mv.get_source())?);
    let capture = board.piece_on(mv.get_dest()).map(convert_piece);

    let from = shakmaty::Square::try_from(mv.get_source().to_index() as u8).unwrap();
    let to = shakmaty::Square::try_from(mv.get_dest().to_index() as u8).unwrap();
    let promotion = mv.get_promotion().map(convert_piece);

    let shak_move = if role == Role::Pawn && capture.is_none() && from.file() != to.file() {
        // en passant
        println!("En PASSANT");
        Move::EnPassant { from, to }
    } else if role == Role::King && from.distance(to) > 1 {
        Move::Castle {
            king: from,
            rook: to,
        }
    } else {
        Move::Normal {
            role,
            capture,
            from,
            to,
            promotion,
        }
    };

    Some(SanPlus::from_move(pos, &shak_move))
}
