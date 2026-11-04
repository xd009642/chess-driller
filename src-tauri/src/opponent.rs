use anyhow::{bail, Context};
use serde::Deserialize;
use std::time::Duration;

#[derive(Clone, Copy, Deserialize)]
#[serde(tag = "kind", rename_all = "snake_case")]
pub enum Opponent {
    Stockfish,
    Lichess {
        rating: u16,
        speed: Speed,
        fallback_skill: u8,
    },
}

#[derive(Clone, Copy, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum Speed {
    Bullet,
    Blitz,
    Rapid,
    Classical,
}

impl Speed {
    fn as_str(self) -> &'static str {
        match self {
            Self::Bullet => "bullet",
            Self::Blitz => "blitz",
            Self::Rapid => "rapid",
            Self::Classical => "classical",
        }
    }
}

impl Opponent {
    pub fn validate(self) -> anyhow::Result<()> {
        if let Self::Lichess {
            rating,
            fallback_skill,
            ..
        } = self
        {
            if ![0, 1000, 1200, 1400, 1600, 1800, 2000, 2200, 2500].contains(&rating) {
                bail!("Choose a supported Lichess rating band");
            }
            if fallback_skill > 20 {
                bail!("Stockfish skill must be between 0 and 20");
            }
        }
        Ok(())
    }

    pub fn label(self) -> String {
        match self {
            Self::Stockfish => "Stockfish".into(),
            Self::Lichess { rating, speed, .. } => {
                let band = match rating {
                    0 => "0–999".into(),
                    2200 => "2200–2499".into(),
                    2500 => "2500+".into(),
                    _ => format!("{rating}–{}", rating + 199),
                };
                format!("Lichess {band} {}", speed.as_str())
            }
        }
    }
}

// A handful of games is too little evidence for a population-style opponent.
// Count continuations, not position visits: some games end at this position.
const MIN_CONTINUATIONS: u64 = 10;

#[derive(Deserialize)]
struct Explorer {
    moves: Vec<ExplorerMove>,
}

#[derive(Deserialize)]
struct ExplorerMove {
    uci: String,
    white: u64,
    draws: u64,
    black: u64,
}

impl Explorer {
    fn choose(self, draw: impl FnOnce(u64) -> u64) -> anyhow::Result<Option<String>> {
        let weights = self
            .moves
            .iter()
            .map(|m| {
                m.white
                    .checked_add(m.draws)
                    .and_then(|n| n.checked_add(m.black))
                    .context("Lichess move count overflow")
            })
            .collect::<anyhow::Result<Vec<_>>>()?;
        let total = weights.iter().try_fold(0u64, |sum, weight| {
            sum.checked_add(*weight)
                .context("Lichess total count overflow")
        })?;
        if total < MIN_CONTINUATIONS {
            return Ok(None);
        }
        let mut ticket = draw(total);
        for (mv, weight) in self.moves.into_iter().zip(weights) {
            if ticket < weight {
                return Ok(Some(mv.uci));
            }
            ticket -= weight;
        }
        bail!("Random draw exceeded the continuation count")
    }
}

pub fn lichess_move(
    fen: &str,
    rating: u16,
    speed: Speed,
    token: &str,
) -> anyhow::Result<Option<String>> {
    if token.trim().is_empty() {
        bail!("Enter a Lichess API token to use the explorer (no permission scopes required)");
    }
    let mut request_gate = crate::provider_http::gate(true)?;
    let client = reqwest::blocking::Client::builder()
        .timeout(Duration::from_secs(15))
        .user_agent("chess-driller")
        .build()?;
    let response = client
        .get("https://explorer.lichess.org/lichess")
        .bearer_auth(token.trim())
        .query(&[
            ("variant", "standard"),
            ("fen", fen),
            ("ratings", &rating.to_string()),
            ("speeds", speed.as_str()),
            // Include uncommon mistakes as well as the default top 12 moves.
            ("moves", "256"),
            ("topGames", "0"),
            ("recentGames", "0"),
        ])
        .send()
        .context("Cannot reach Lichess; check your connection and retry")?;
    if response.status() == reqwest::StatusCode::UNAUTHORIZED {
        bail!("Lichess rejected the token. Enter a valid API token and retry");
    }
    if response.status() == reqwest::StatusCode::TOO_MANY_REQUESTS {
        crate::provider_http::status(&response, &mut request_gate)?;
    }
    response
        .error_for_status()
        .context("Lichess request failed")?
        .json::<Explorer>()
        .context("Invalid Lichess explorer response")?
        .choose(|total| fastrand::u64(..total))
}

#[cfg(test)]
mod tests {
    use super::*;

    /// The explorer contract counts each move by white wins + draws + black
    /// wins. Enumerating every ticket verifies exact frequency without a flaky
    /// random sample. Reordering moves and changing the outcome mix are valid.
    #[test]
    fn sampling_counts_games_not_wins_or_move_order() {
        for (first, second) in [("e7e5", "c7c5"), ("c7c5", "e7e5")] {
            for counts in [[2, 3, 1], [0, 0, 6], [6, 0, 0]] {
                let mut replies = Vec::new();
                for ticket in 0..10 {
                    // Synthetic response using the documented explorer schema.
                    let data = serde_json::json!({"moves": [
                        {"uci": "a7a6", "white": 0, "draws": 0, "black": 0},
                        {"uci": first, "white": counts[0], "draws": counts[1], "black": counts[2]},
                        {"uci": second, "white": 1, "draws": 2, "black": 1}
                    ]});
                    let explorer: Explorer = serde_json::from_value(data).expect("Explorer schema");
                    replies.push(
                        explorer
                            .choose(|total| {
                                assert_eq!(total, 10);
                                ticket
                            })
                            .expect("Select move")
                            .expect("Enough games"),
                    );
                }
                assert_eq!(replies.iter().filter(|m| m.as_str() == first).count(), 6);
                assert_eq!(replies.iter().filter(|m| m.as_str() == second).count(), 4);
            }
        }
    }

    /// Sparse positions hand over without drawing from an empty range. Ten
    /// continuations is the product cutoff, independent of games ending here.
    #[test]
    fn sparse_positions_hand_over_at_the_cutoff() {
        for count in [0, 9, 10, 11] {
            let data = serde_json::json!({"white": 1000, "draws": 1000, "black": 1000,
                "moves": [{"uci": "e7e5", "white": count, "draws": 0, "black": 0}]});
            let explorer: Explorer = serde_json::from_value(data).expect("Explorer schema");
            let result = explorer
                .choose(|_| {
                    assert!(count >= 10, "Sparse positions must not sample");
                    0
                })
                .expect("Choose or hand over");
            assert_eq!(result.as_deref(), (count >= 10).then_some("e7e5"));
        }
        let empty = Explorer { moves: vec![] };
        assert!(empty
            .choose(|_| panic!("No moves to sample"))
            .expect("Empty response")
            .is_none());
    }

    /// UI settings are untrusted IPC input. Valid bands include the irregular
    /// top bands; arbitrary ratings must not silently select a different band.
    #[test]
    fn validates_explorer_settings() {
        for rating in [0, 1000, 1200, 1400, 1600, 1800, 2000, 2200, 2500] {
            for fallback_skill in [0, 10, 20] {
                Opponent::Lichess {
                    rating,
                    speed: Speed::Rapid,
                    fallback_skill,
                }
                .validate()
                .expect("Supported settings");
            }
        }
        for (rating, fallback_skill) in [(1199, 0), (2600, 0), (1200, 21)] {
            assert!(Opponent::Lichess {
                rating,
                speed: Speed::Blitz,
                fallback_skill
            }
            .validate()
            .is_err());
        }
    }
}
