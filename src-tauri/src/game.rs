use crate::repertoire::{Repertoire, Source};
use anyhow::{bail, Context};
use serde::Serialize;
use shakmaty::{
    fen::Fen, san::SanPlus, uci::Uci, CastlingMode, Chess, Color, EnPassantMode, Position,
};

#[derive(Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum Phase {
    Prep,
    Deviated,
    Engine,
    Finished,
}

#[derive(Serialize)]
pub struct View {
    pub fen: String,
    pub color: String,
    pub phase: Phase,
    pub message: String,
    pub sources: Vec<Source>,
    pub alternatives: Vec<String>,
    pub moves: Vec<String>,
    pub needs_engine: bool,
}

pub struct Session {
    position: Chess,
    player: Color,
    tree: Repertoire,
    node: usize,
    phase: Phase,
    message: String,
    pub history: Vec<String>,
    moves: Vec<String>,
    retry: Option<Chess>,
    alternatives: Vec<String>,
}

impl Session {
    pub fn new(tree: Repertoire, player: Color) -> anyhow::Result<Self> {
        let mut session = Self {
            position: Chess::default(),
            player,
            tree,
            node: 0,
            phase: Phase::Prep,
            message: "Your move — stay in your prep.".into(),
            history: Vec::new(),
            moves: Vec::new(),
            retry: None,
            alternatives: Vec::new(),
        };
        session.advance()?;
        Ok(session)
    }

    pub fn view(&self) -> View {
        View {
            fen: Fen::from_position(self.position.clone(), EnPassantMode::Legal).to_string(),
            color: self.player.to_string(),
            phase: self.phase,
            message: self.message.clone(),
            sources: self.tree.nodes[self.node].sources.clone(),
            alternatives: self.alternatives.clone(),
            moves: self.moves.clone(),
            needs_engine: self.phase == Phase::Engine && self.position.turn() != self.player,
        }
    }

    fn play(&mut self, uci: &str) -> anyhow::Result<()> {
        let mv = uci.parse::<Uci>()?.to_move(&self.position)?;
        self.moves
            .push(SanPlus::from_move(self.position.clone(), &mv).to_string());
        self.history
            .push(Uci::from_move(&mv, CastlingMode::Standard).to_string());
        self.position.play_unchecked(&mv);
        Ok(())
    }

    pub fn player_move(&mut self, uci: &str) -> anyhow::Result<()> {
        if !matches!(self.phase, Phase::Prep | Phase::Engine) || self.position.turn() != self.player
        {
            bail!("It is not your turn");
        }
        // Validate before changing the session, including the source square and promotion.
        let mv = uci.parse::<Uci>()?.to_move(&self.position)?;
        let canonical = Uci::from_move(&mv, CastlingMode::Standard).to_string();
        if self.phase == Phase::Prep {
            let next = self.tree.nodes[self.node]
                .children
                .iter()
                .find(|(m, _)| *m == canonical)
                .map(|(_, n)| *n);
            if let Some(next) = next {
                self.play(&canonical)?;
                self.node = next;
            } else {
                let alternatives = self.tree.nodes[self.node]
                    .children
                    .iter()
                    .map(|(m, _)| {
                        let mv = m.parse::<Uci>()?.to_move(&self.position)?;
                        Ok(SanPlus::from_move(self.position.clone(), &mv).to_string())
                    })
                    .collect::<anyhow::Result<Vec<_>>>()?;
                self.retry = Some(self.position.clone());
                self.play(&canonical)?;
                self.phase = Phase::Deviated;
                self.alternatives = alternatives;
                self.message = format!("{} is outside your selected prep. Retry the move or continue against Stockfish.", self.moves.last().context("Move was not recorded")?);
                return Ok(());
            }
        } else {
            self.play(&canonical)?;
        }
        self.advance()
    }

    pub fn retry(&mut self) -> anyhow::Result<()> {
        if self.phase != Phase::Deviated {
            bail!("There is no deviation to retry");
        }
        self.position = self
            .retry
            .take()
            .context("Missing position before deviation")?;
        self.moves.pop();
        self.history.pop();
        self.alternatives.clear();
        self.phase = Phase::Prep;
        self.message = "Try again — choose a prepared move.".into();
        Ok(())
    }

    pub fn continue_engine(&mut self) -> anyhow::Result<()> {
        if self.phase != Phase::Deviated {
            bail!("The drill has not deviated from prep");
        }
        self.phase = Phase::Engine;
        self.retry = None;
        self.alternatives.clear();
        self.message = "Outside prep — playing Stockfish.".into();
        self.advance()
    }

    pub fn engine_move(&mut self, uci: &str) -> anyhow::Result<()> {
        if !self.view().needs_engine {
            bail!("No engine move is needed");
        }
        self.play(uci)
            .context("Stockfish returned an illegal move")?;
        self.advance()
    }

    fn finished(&mut self) -> bool {
        if let Some(outcome) = self.position.outcome() {
            self.phase = Phase::Finished;
            self.message = format!("Game over: {outcome}");
            true
        } else {
            false
        }
    }

    fn advance(&mut self) -> anyhow::Result<()> {
        if self.finished() {
            return Ok(());
        }
        if self.phase == Phase::Prep {
            if self.tree.nodes[self.node].children.is_empty() {
                self.phase = Phase::Engine;
                self.message = "Prep complete — continuing against Stockfish.".into();
            } else if self.position.turn() != self.player {
                let (mv, next) = fastrand::choice(self.tree.nodes[self.node].children.iter())
                    .context("Prepared reply is missing")?
                    .clone();
                self.play(&mv)?;
                self.node = next;
                if self.finished() {
                    return Ok(());
                }
                if self.tree.nodes[self.node].children.is_empty() {
                    self.phase = Phase::Engine;
                    self.message = "Prep complete — continuing against Stockfish.".into();
                } else {
                    self.message = "In prep — your move.".into();
                }
            }
        }
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::repertoire::PrepFile;

    fn session(pgn: &str, player: Color) -> Session {
        let tree = Repertoire::from_files(&[PrepFile {
            id: 1,
            name: "Study.pgn".into(),
            color: player.to_string(),
            pgn: pgn.into(),
        }])
        .expect("Valid prep");
        Session::new(tree, player).expect("Start drill")
    }

    /// Handover happens at the leaf itself, whether the player or the opponent
    /// plays the last prepared move. Changing line length must remain valid.
    #[test]
    fn handover_on_either_sides_last_move() {
        let mut white = session("1. e4 *", Color::White);
        white.player_move("e2e4").expect("Prepared move");
        assert!(matches!(white.phase, Phase::Engine));
        assert!(white.view().needs_engine);
        white.engine_move("e7e5").expect("Legal engine reply");
        assert!(!white.view().needs_engine);

        let mut white = session("1. e4 e5 *", Color::White);
        white.player_move("e2e4").expect("Prepared move");
        assert_eq!(white.history, ["e2e4", "e7e5"]);
        assert!(matches!(white.phase, Phase::Engine));
        assert!(!white.view().needs_engine);
        white.player_move("g1f3").expect("Play after prep");
        assert!(white.view().needs_engine);

        let mut black = session("1. e4 e5 *", Color::Black);
        assert_eq!(black.history, ["e2e4"]);
        black.player_move("e7e5").expect("Black prepared move");
        assert!(black.view().needs_engine);
    }

    /// A deviation pauses before the opponent replies. Retry restores the exact
    /// position; continuing retains the deviation and full engine history.
    #[test]
    fn deviation_retry_and_continue() {
        let mut drill = session("1. e4 e5 2. Nf3 *", Color::White);
        let before = drill.view().fen;
        drill.player_move("d2d4").expect("Legal deviation");
        assert!(matches!(drill.phase, Phase::Deviated));
        assert_eq!(drill.view().alternatives, ["e4"]);
        assert_eq!(drill.history, ["d2d4"]);
        assert!(!drill.view().needs_engine);
        drill.retry().expect("Retry deviation");
        assert_eq!(drill.view().fen, before);
        assert!(drill.history.is_empty());
        drill.player_move("e2e4").expect("Correct retry");
        assert_eq!(drill.history, ["e2e4", "e7e5"]);
        drill.player_move("b1c3").expect("Another deviation");
        drill.continue_engine().expect("Continue with engine");
        assert!(drill.view().needs_engine);
        assert_eq!(drill.history, ["e2e4", "e7e5", "b1c3"]);
    }

    /// User input must identify the actual source square, not just piece type
    /// and destination. Legal alternatives must still work after a rejection.
    #[test]
    fn illegal_moves_leave_position_unchanged() {
        let mut drill = session("1. e4 e5 2. Nf3 *", Color::White);
        let before = drill.view().fen;
        for mv in ["b1f3", "e7e5", "e2e5", "garbage", "e2e4q"] {
            assert!(drill.player_move(mv).is_err());
            assert_eq!(drill.view().fen, before);
            assert!(drill.history.is_empty());
        }
        drill
            .player_move("e2e4")
            .expect("Legal move after rejection");
    }

    /// UCI conversion must preserve castling and the selected promotion role.
    #[test]
    fn castling_and_underpromotion() {
        let mut drill = session("1. e4 e5 2. Nf3 Nc6 3. Bc4 Bc5 4. O-O *", Color::White);
        for mv in ["e2e4", "g1f3", "f1c4", "e1g1"] {
            drill.player_move(mv).expect("Prepared castling line");
        }
        assert_eq!(drill.moves.last().expect("Last move"), "O-O");
        assert!(matches!(drill.phase, Phase::Engine));
        let mut drill = session(
            "1. a4 h5 2. a5 h4 3. a6 h3 4. axb7 hxg2 5. bxa8=N *",
            Color::White,
        );
        for mv in ["a2a4", "a4a5", "a5a6", "a6b7", "b7a8n"] {
            drill.player_move(mv).expect("Prepared promotion line");
        }
        assert!(matches!(drill.phase, Phase::Engine));
        assert_eq!(drill.history.last().expect("Last move"), "b7a8n");
    }

    /// A terminal position takes precedence over engine handover.
    #[test]
    fn checkmate_does_not_request_engine() {
        let mut drill = session("1. f3 e5 2. g4 Qh4# *", Color::White);
        drill.player_move("f2f3").expect("First move");
        drill.player_move("g2g4").expect("Mating reply");
        assert!(matches!(drill.phase, Phase::Finished));
        assert!(!drill.view().needs_engine);
    }
}
