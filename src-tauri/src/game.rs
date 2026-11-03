use crate::{
    opponent::Opponent,
    repertoire::{Repertoire, Source},
};
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
    pub opponent: String,
    pub fallback: bool,
    pub uses_lichess: bool,
}

pub struct Session {
    pub(crate) opponent: Opponent,
    pub(crate) fallback: bool,
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
        Self::with_opponent(tree, player, Opponent::Stockfish)
    }

    pub(crate) fn with_opponent(
        tree: Repertoire,
        player: Color,
        opponent: Opponent,
    ) -> anyhow::Result<Self> {
        opponent.validate()?;
        let mut session = Self {
            opponent,
            fallback: false,
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
            opponent: self.opponent_label(),
            fallback: self.fallback,
            uses_lichess: matches!(self.opponent, Opponent::Lichess { .. }) && !self.fallback,
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
                self.message = format!(
                    "{} is outside your selected prep. Retry the move or continue against {}.",
                    self.moves.last().context("Move was not recorded")?,
                    self.opponent_label()
                );
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
        self.message = format!("Outside prep — playing {}.", self.opponent_label());
        self.advance()
    }

    pub fn engine_move(&mut self, uci: &str) -> anyhow::Result<()> {
        if !self.view().needs_engine {
            bail!("No engine move is needed");
        }
        self.play(uci)
            .context("Opponent returned an illegal move")?;
        self.advance()
    }

    fn opponent_label(&self) -> String {
        match self.opponent {
            Opponent::Lichess { fallback_skill, .. } if self.fallback => {
                format!("Stockfish (skill {fallback_skill})")
            }
            opponent => opponent.label(),
        }
    }

    pub(crate) fn use_stockfish(&mut self) -> anyhow::Result<()> {
        if !self.view().needs_engine
            || self.fallback
            || !matches!(self.opponent, Opponent::Lichess { .. })
        {
            bail!("No Lichess handover is pending");
        }
        self.fallback = true;
        self.message = format!("Continuing against {}.", self.opponent_label());
        Ok(())
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
                self.message = format!(
                    "Prep complete — continuing against {}.",
                    self.opponent_label()
                );
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
                    self.message = format!(
                        "Prep complete — continuing against {}.",
                        self.opponent_label()
                    );
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
        session_with_opponent(pgn, player, Opponent::Stockfish)
    }

    fn session_with_opponent(pgn: &str, player: Color, opponent: Opponent) -> Session {
        let tree = Repertoire::from_files(&[PrepFile {
            id: 1,
            name: "Study.pgn".into(),
            color: player.to_string(),
            pgn: pgn.into(),
        }])
        .expect("Valid prep");
        Session::with_opponent(tree, player, opponent).expect("Start drill")
    }

    /// A Lichess reply uses the same legal-move and turn rules as engine play.
    /// Prep can end on either side's move; changing rating and fallback skill
    /// must not alter that handover or discard the accumulated history.
    #[test]
    fn lichess_handover_preserves_position_and_turn() {
        use crate::opponent::Speed;
        for player in [Color::White, Color::Black] {
            for (rating, fallback_skill) in [(1200, 0), (2200, 10)] {
                let opponent = Opponent::Lichess {
                    rating,
                    speed: Speed::Rapid,
                    fallback_skill,
                };
                let mut drill = session_with_opponent("1. e4 e5 *", player, opponent);
                drill
                    .player_move(if player == Color::White {
                        "e2e4"
                    } else {
                        "e7e5"
                    })
                    .expect("Finish prep");
                assert_eq!(drill.history, ["e2e4", "e7e5"]);
                assert_eq!(drill.view().opponent, opponent.label());
                assert!(!drill.view().fallback);
                if player == Color::White {
                    drill.player_move("g1f3").expect("Player continues");
                }
                assert!(drill.view().needs_engine);
                let before = drill.view().fen;
                let history = drill.history.clone();
                assert!(drill.engine_move("e2e4").is_err());
                assert_eq!(drill.view().fen, before);
                assert_eq!(drill.history, history);
                drill.use_stockfish().expect("Sparse data handover");
                assert_eq!(drill.view().fen, before);
                assert_eq!(drill.history, history);
                assert!(drill.view().needs_engine);
                assert!(drill.view().fallback);
                assert_eq!(
                    drill.view().opponent,
                    format!("Stockfish (skill {fallback_skill})")
                );
                drill
                    .engine_move(if player == Color::White {
                        "b8c6"
                    } else {
                        "g1f3"
                    })
                    .expect("Fallback reply");
                assert!(!drill.view().needs_engine);
                assert!(drill.view().fallback);
                assert_eq!(&drill.history[..history.len()], history);
                drill
                    .player_move(if player == Color::White {
                        "f1c4"
                    } else {
                        "b8c6"
                    })
                    .expect("Continue conversion practice");
                assert!(drill.view().fallback);
                assert!(drill.view().needs_engine);
            }
        }
    }

    /// Deviating must retain the selected opponent, and retry must still restore
    /// prep. A subsequent Lichess reply can end the game without an engine call.
    #[test]
    fn lichess_deviation_retry_and_checkmate() {
        let opponent = Opponent::Lichess {
            rating: 1200,
            speed: crate::opponent::Speed::Blitz,
            fallback_skill: 0,
        };
        let mut drill = session_with_opponent("1. e4 *", Color::White, opponent);
        let before = drill.view().fen;
        drill.player_move("f2f3").expect("Deviation");
        assert!(!drill.view().needs_engine);
        drill.retry().expect("Retry");
        assert_eq!(drill.view().fen, before);
        drill.player_move("f2f3").expect("Deviation again");
        drill
            .continue_engine()
            .expect("Continue with selected opponent");
        assert_eq!(drill.view().opponent, opponent.label());
        assert!(drill.view().needs_engine);
        drill.engine_move("e7e5").expect("Lichess reply");
        drill.player_move("g2g4").expect("Player blunder");
        drill.engine_move("d8h4").expect("Lichess mating reply");
        assert!(matches!(drill.view().phase, Phase::Finished));
        assert!(!drill.view().needs_engine);
        assert!(!drill.view().fallback);
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
