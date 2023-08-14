//!
//! List of archives we want to process:
//! GET https://api.chess.com/pub/player/$USER/games/archives
//!
//! All PGNs for a month
//! "https://api.chess.com/pub/player/$USER/games/$YEAR/$MONTH/pgn" year and month are numbers
use reqwest::blocking::*;
use serde::Deserialize;

#[derive(Clone)]
pub struct ChessComClient {
    client: Client,
}

#[derive(Debug, Clone, Deserialize)]
pub struct Archives {
    archives: Vec<String>,
}

impl ChessComClient {
    pub fn new() -> Self {
        Self {
            client: Client::new(),
        }
    }

    pub fn get_user_archives(&self, user: &str) -> anyhow::Result<Vec<String>> {
        let resp = self
            .client
            .get(format!(
                "https://api.chess.com/pub/player/{}/games/archives",
                user
            ))
            .send()?
            .json::<Archives>()?;

        Ok(resp.archives)
    }

    pub fn download_pgn_archive(&self, user: &str, year: u16, month: u8) -> anyhow::Result<String> {
        self.download_pgn(&format!(
            "https://api.chess.com/pub/player/{}/games/{}/{}/pgn",
            user, year, month
        ))
    }

    pub fn download_pgn(&self, url: &str) -> anyhow::Result<String> {
        let resp = self.client.get(url).send()?.text()?;

        Ok(resp)
    }
}
