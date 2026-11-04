//! Review uses the same prefix tree as practice, but never mutates a drill.
use crate::repertoire::{Repertoire, Source};
use anyhow::{bail, Context};
use pgn_reader::{BufferedReader, RawHeader, SanPlus, Skip, Visitor};
use serde::{Deserialize, Serialize};
use shakmaty::{fen::Fen, san::San, uci::Uci, CastlingMode, Chess, Color, EnPassantMode, Position};

#[derive(Clone, Serialize, Deserialize)]
pub struct PlayedMove {
    pub uci: String,
    pub san: String,
    pub before: String,
    pub after: String,
}

#[derive(Default)]
struct Mainline {
    position: Chess,
    moves: Vec<PlayedMove>,
    games: usize,
    error: Option<String>,
}

pub fn fen(position: &Chess) -> String {
    Fen::from_position(position.clone(), EnPassantMode::Legal).to_string()
}

pub fn numbered(san: impl std::fmt::Display, ply: usize) -> String {
    format!(
        "{}{} {san}",
        ply / 2 + 1,
        if ply % 2 == 0 { "." } else { "..." }
    )
}

impl Visitor for Mainline {
    type Result = ();
    fn begin_game(&mut self) {
        self.games += 1;
    }
    fn header(&mut self, key: &[u8], value: RawHeader<'_>) {
        let value = value.decode_utf8_lossy();
        if (key == b"FEN" && value != fen(&Chess::default()))
            || (key == b"Variant" && !matches!(value.as_ref(), "Standard" | "Chess"))
        {
            self.error = Some("Unsupported variant or starting position".into());
        }
    }
    fn begin_variation(&mut self) -> Skip {
        Skip(true)
    }
    fn san(&mut self, san: SanPlus) {
        if self.error.is_some() {
            return;
        }
        match san.san.to_move(&self.position) {
            Ok(mv) => {
                let before = fen(&self.position);
                let san = numbered(San::from_move(&self.position, &mv), self.moves.len());
                let uci = Uci::from_move(&mv, CastlingMode::Standard).to_string();
                self.position.play_unchecked(&mv);
                self.moves.push(PlayedMove {
                    uci,
                    san,
                    before,
                    after: fen(&self.position),
                });
            }
            Err(error) => self.error = Some(format!("Illegal mainline move {san}: {error}")),
        }
    }
    fn end_game(&mut self) {}
}

// pgn-reader deliberately skips unrecognised tokens. That is useful for study
// imports but unsafe for review: dropping a malformed played move invents a line.
// Check mainline tokens and delimiters before delegating chess legality to shakmaty.
fn validate_tokens(pgn: &str) -> anyhow::Result<usize> {
    let bytes = pgn.trim_start_matches('\u{feff}').as_bytes();
    let mut index = 0;
    let mut depth = 0usize;
    let mut moves = 0;
    while index < bytes.len() {
        match bytes[index] {
            b if b.is_ascii_whitespace() => index += 1,
            b';' | b'%' => {
                while index < bytes.len() && bytes[index] != b'\n' {
                    index += 1;
                }
            }
            b'{' => {
                index += 1;
                while index < bytes.len() && bytes[index] != b'}' {
                    index += 1;
                }
                if index == bytes.len() {
                    bail!("Unclosed PGN comment");
                }
                index += 1;
            }
            b'[' => {
                index += 1;
                let mut quoted = false;
                while index < bytes.len() {
                    match bytes[index] {
                        b'\\' if quoted => {
                            index += 2;
                            continue;
                        }
                        b'"' => quoted = !quoted,
                        b']' if !quoted => break,
                        _ => {}
                    }
                    index += 1;
                }
                if index >= bytes.len() {
                    bail!("Unclosed PGN header");
                }
                index += 1;
            }
            b'(' => {
                depth += 1;
                index += 1;
            }
            b')' => {
                depth = depth.checked_sub(1).context("Unbalanced PGN variation")?;
                index += 1;
            }
            b'}' | b']' => bail!("Unbalanced PGN delimiter"),
            b'$' => {
                index += 1;
                let start = index;
                while index < bytes.len() && bytes[index].is_ascii_digit() {
                    index += 1;
                }
                if start == index {
                    bail!("Invalid PGN annotation");
                }
            }
            b'!' | b'?' => index += 1,
            _ => {
                let start = index;
                while index < bytes.len()
                    && !bytes[index].is_ascii_whitespace()
                    && !b"{}[]();$!?".contains(&bytes[index])
                {
                    index += 1;
                }
                let mut token = &bytes[start..index];
                if depth > 0 || matches!(token, b"1-0" | b"0-1" | b"1/2-1/2" | b"*") {
                    continue;
                }
                let number = token.iter().take_while(|b| b.is_ascii_digit()).count();
                if token.get(number) == Some(&b'.') {
                    token = &token[number..];
                    token = token
                        .strip_prefix(b"...")
                        .or_else(|| token.strip_prefix(b"."))
                        .context("Invalid move number")?;
                }
                if token.is_empty() || token == b"..." {
                    continue;
                }
                let normalized = if token.starts_with(b"0-0") {
                    token
                        .iter()
                        .map(|b| if *b == b'0' { b'O' } else { *b })
                        .collect::<Vec<_>>()
                } else {
                    token.to_vec()
                };
                SanPlus::from_ascii(&normalized).map_err(|_| {
                    anyhow::anyhow!(
                        "Malformed mainline token: {}",
                        String::from_utf8_lossy(token)
                    )
                })?;
                moves += 1;
            }
        }
    }
    if depth != 0 {
        bail!("Unclosed PGN variation");
    }
    Ok(moves)
}

pub fn parse(pgn: &str) -> anyhow::Result<Vec<PlayedMove>> {
    let expected_moves = validate_tokens(pgn)?;
    let mut visitor = Mainline::default();
    BufferedReader::new(pgn.as_bytes()).read_all(&mut visitor)?;
    if let Some(error) = visitor.error {
        bail!("{error}");
    }
    if visitor.games != 1 {
        bail!("Expected one game in the PGN");
    }
    if visitor.moves.len() != expected_moves {
        bail!("PGN mainline could not be read completely");
    }
    Ok(visitor.moves)
}

#[derive(Serialize)]
pub struct Alternative {
    pub san: String,
    pub uci: String,
    pub sources: Vec<Source>,
}

#[derive(Serialize)]
pub struct Review {
    pub classification: &'static str,
    pub matched_ply: usize,
    pub departure_ply: Option<usize>,
    pub fen: String,
    pub played: Option<PlayedMove>,
    pub alternatives: Vec<Alternative>,
    pub sources: Vec<Source>,
}

pub fn compare(
    moves: &[PlayedMove],
    color: Color,
    tree: Option<&Repertoire>,
) -> anyhow::Result<Review> {
    let mut position = Chess::default();
    let mut review = Review {
        classification: "no_prep",
        matched_ply: 0,
        departure_ply: None,
        fen: fen(&position),
        played: None,
        alternatives: vec![],
        sources: vec![],
    };
    let Some(tree) = tree else {
        return Ok(review);
    };
    let mut node = 0;
    for (ply, played) in moves.iter().enumerate() {
        review.sources = tree.nodes[node].sources.clone();
        review.fen = fen(&position);
        if tree.nodes[node].children.is_empty() {
            review.classification = "prep_completed";
            return Ok(review);
        }
        if let Some((_, next)) = tree.nodes[node]
            .children
            .iter()
            .find(|(uci, _)| *uci == played.uci)
        {
            let mv = played
                .uci
                .parse::<Uci>()?
                .to_move(&position)
                .context("Invalid validated game move")?;
            position.play_unchecked(&mv);
            node = *next;
            review.matched_ply += 1;
        } else {
            review.classification = if position.turn() == color {
                "your_deviation"
            } else {
                "opponent_left_prep"
            };
            review.departure_ply = Some(ply + 1);
            review.played = Some(played.clone());
            for (uci, child) in &tree.nodes[node].children {
                let mv = uci
                    .parse::<Uci>()?
                    .to_move(&position)
                    .context("Invalid prep move")?;
                review.alternatives.push(Alternative {
                    san: numbered(San::from_move(&position, &mv), ply),
                    uci: uci.clone(),
                    sources: tree.nodes[*child].sources.clone(),
                });
            }
            return Ok(review);
        }
    }
    review.fen = fen(&position);
    review.sources = tree.nodes[node].sources.clone();
    review.classification = if tree.nodes[node].children.is_empty() {
        "prep_completed"
    } else {
        "game_ended_in_prep"
    };
    Ok(review)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::repertoire::PrepFile;
    fn file(id: u64, pgn: &str) -> PrepFile {
        PrepFile {
            id,
            name: format!("{id}.pgn"),
            color: "white".into(),
            pgn: pgn.into(),
        }
    }
    fn review(pgn: &str, color: Color, files: &[PrepFile]) -> Review {
        let moves = parse(pgn).expect("Legal fixture mainline");
        let tree = Repertoire::from_files(files).expect("Legal prep");
        compare(&moves, color, Some(&tree)).expect("Compare legal moves")
    }

    /// The plan's hand-checked sequence distinguishes the actor at the first
    /// absent edge. Adding a selected alternative must remove that departure.
    #[test]
    fn departure_actor_and_selected_alternatives() {
        let prep = file(1, "1. e4 e5 2. Nf3 Nc6 *");
        for (color, own, other) in [
            (Color::White, "your_deviation", "opponent_left_prep"),
            (Color::Black, "opponent_left_prep", "your_deviation"),
        ] {
            let result = review("1. e4 e5 2. Bc4 Nc6 *", color, &[prep.clone()]);
            assert_eq!(result.classification, own);
            assert_eq!(result.departure_ply, Some(3));
            assert_eq!(result.matched_ply, 2);
            assert_eq!(result.alternatives[0].san, "2. Nf3");
            assert_eq!(
                result.fen,
                "rnbqkbnr/pppp1ppp/8/4p3/4P3/8/PPPP1PPP/RNBQKBNR w KQkq - 0 2"
            );
            let result = review("1. e4 c5 *", color, &[prep.clone()]);
            assert_eq!(result.classification, other);
            assert_eq!(result.played.expect("Departure").san, "1... c5");
            let result = review("1. d4 d5 *", color, &[prep.clone()]);
            assert_eq!(result.classification, own);
            assert_eq!(result.departure_ply, Some(1));
        }
        let added = file(2, "1. e4 e5 2. Bc4 *");
        assert_eq!(
            review("1. e4 e5 2. Bc4 *", Color::White, &[prep, added]).classification,
            "prep_completed"
        );
    }

    /// A short chapter must not hide selected continuations. A game ending at
    /// a leaf completes prep, while a game ending before it is still in prep.
    #[test]
    fn leaves_overlaps_and_no_selection() {
        let short = file(1, "1. e4 e5 *");
        let long = file(2, "1. e4 e5 2. Nf3 Nc6 *");
        assert_eq!(
            review("1. e4 e5 *", Color::White, &[short.clone()]).classification,
            "prep_completed"
        );
        assert_eq!(
            review("1. e4 e5 *", Color::White, &[short, long.clone()]).classification,
            "game_ended_in_prep"
        );
        for pgn in ["1. e4 e5 2. Nf3 Nc6 *", "1. e4 e5 2. Nf3 Nc6 3. Bc4 *"] {
            assert_eq!(
                review(pgn, Color::White, &[long.clone()]).classification,
                "prep_completed"
            );
        }
        assert_eq!(
            compare(&parse("1. e4 *").expect("Legal game"), Color::White, None)
                .expect("No selection")
                .classification,
            "no_prep"
        );
    }

    /// Child sources must exclude unrelated chapters at the shared parent.
    /// Reordering files and nested variations must preserve these identities.
    #[test]
    fn alternative_sources_are_specific_to_each_edge() {
        let a = file(
            1,
            "[Event \"Open\"]\n\n1. e4 e5 (1... c5 2. Nf3 (2. Nc3)) *",
        );
        let b = file(2, "[Event \"French\"]\n\n1. e4 e6 *");
        for files in [vec![a.clone(), b.clone()], vec![b.clone(), a.clone()]] {
            let result = review("1. e4 d5 *", Color::Black, &files);
            assert_eq!(result.sources.len(), 2);
            for alternative in result.alternatives {
                let expected = if alternative.uci == "e7e6" { 2 } else { 1 };
                assert_eq!(alternative.sources.len(), 1);
                assert_eq!(alternative.sources[0].file_id, expected);
            }
            assert_eq!(
                review("1. e4 c5 2. Nc3 *", Color::White, &files).classification,
                "prep_completed"
            );
        }
        assert_eq!(
            review("1. e4 e6 *", Color::Black, &[a]).classification,
            "your_deviation"
        );
    }

    /// Returning to a familiar position after a departure must not rematch.
    /// A different selected move order may legitimately accept the same game.
    #[test]
    fn no_transpositions_or_rematching() {
        let prep = file(1, "1. Nf3 d5 2. d4 Nf6 *");
        let result = review("1. d4 d5 2. Nf3 Nf6 *", Color::White, &[prep]);
        assert_eq!(result.departure_ply, Some(1));
        assert_eq!(result.matched_ply, 0);
    }

    /// PGN comments, annotations and RAVs must not change the played line;
    /// an illegal move after a departure must invalidate the entire game.
    #[test]
    fn validates_entire_mainline_and_ignores_variations() {
        let moves = parse("1. e4! {note} (1. d4 d5) e5 $1 2. Nf3 Nc6 3. Bc4 Nf6 4. O-O *")
            .expect("Annotated legal game");
        assert_eq!(moves.len(), 7);
        assert_eq!(moves[6].uci, "e1g1");
        for invalid in [
            "1. e4 garbage e5 *",
            "1. e4 (1. d4 *",
            "1. e4 c5 2. Bc4 e5 3. Ke4 *",
            "[Variant \"Atomic\"]\n\n1. e4 *",
            "[FEN \"8/8/8/8/8/8/8/K6k w - - 0 1\"]\n\n*",
            "1. e4 *\n\n1. d4 *",
            "",
        ] {
            assert!(parse(invalid).is_err(), "Accepted {invalid}");
        }
    }

    /// Standard UCI encodes en passant by its destination and promotion with
    /// the selected piece; castling and promotion cannot be inferred from SAN text.
    #[test]
    fn special_moves_from_normal_start() {
        let moves = parse("1. e4 a6 2. e5 d5 3. exd6 *").expect("En passant");
        assert_eq!(moves[4].uci, "e5d6");
        let moves =
            parse("1. a4 h5 2. a5 h4 3. a6 h3 4. axb7 hxg2 5. bxa8=N *").expect("Underpromotion");
        assert_eq!(moves[8].uci, "b7a8n");
    }
}
