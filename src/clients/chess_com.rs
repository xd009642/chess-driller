//!
//! List of archives we want to process:
//! GET https://api.chess.com/pub/player/$USER/games/archives
//!
//! All PGNs for a month
//! "https://api.chess.com/pub/player/$USER/games/$YEAR/$MONTH/pgn" year and month are numbers
use crate::db::OpeningDatabase;
use chrono::Datelike;
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

    pub fn download_pgn_archives_after(
        &self,
        user: &str,
        year: u16,
        month: u8,
    ) -> anyhow::Result<OpeningDatabase> {
        let current = chrono::Utc::now();

        let mut db = OpeningDatabase::default();

        for y in year..(current.year() as u16 + 1) {
            let start_month = if y == year { month } else { 1 };
            let end_month = if y == current.year() as u16 {
                current.month() as u8 + 1
            } else {
                13
            };
            for m in start_month..end_month {
                let pgn = self.download_pgn(&archive_url(user, y, m))?;
                db.add_multigame_pgn(pgn.as_bytes(), user.to_string())?;
            }
        }
        Ok(db)
    }

    pub fn download_pgn_archive(
        &self,
        user: &str,
        year: u16,
        month: u8,
    ) -> anyhow::Result<OpeningDatabase> {
        let pgn = self.download_pgn(&archive_url(user, year, month))?;
        OpeningDatabase::load_multigame_pgn(pgn.as_bytes(), user.to_string())
    }

    pub fn download_pgn(&self, url: &str) -> anyhow::Result<String> {
        let resp = self.client.get(url).send()?.text()?;

        Ok(resp)
    }
}

fn archive_url(user: &str, year: u16, month: u8) -> String {
    format!(
        "https://api.chess.com/pub/player/{}/games/{}/{}/pgn",
        user, year, month
    )
}
