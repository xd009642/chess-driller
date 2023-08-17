//!
//! List of archives we want to process:
//! GET https://api.chess.com/pub/player/$USER/games/archives
//!
//! All PGNs for a month
//! "https://api.chess.com/pub/player/$USER/games/$YEAR/$MONTH/pgn" year and month are numbers
use crate::config::Config;
use crate::db::OpeningDatabase;
use chrono::Datelike;
use reqwest::blocking::*;
use serde::Deserialize;
use std::borrow::Cow;
use std::fs;
use tracing::{error, info};

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
            // So with the default user agent you trigger chess.com's security gateway and it sends
            // back an HTML page telling you that you look dangerous. It does trust curl though
            // with no caveats.
            client: Client::builder().user_agent("curl/7.58.0").build().unwrap(),
        }
    }

    pub fn download_all_games(&self, config: &Config) -> OpeningDatabase {
        let mut db = OpeningDatabase::default();
        let chess_com_games = config.data_dir().join("chess.com");
        for user in &config.chess_com {
            let archives = match self.get_user_archives(user) {
                Ok(a) => a,
                Err(e) => {
                    error!("Couldn't get player archives for {}: {}", user, e);
                    continue;
                }
            };
            let user_folder = chess_com_games.join(user);
            if user_folder.exists() {
                info!("Skipping download you already have games for {}", user);
                continue;
            } else {
                fs::create_dir_all(&user_folder).unwrap();
            }
            for (i, archive) in archives.iter().enumerate() {
                let archive = if archive.ends_with("/pgn") {
                    Cow::Borrowed(archive)
                } else {
                    let mut s = archive.to_string();
                    if !s.ends_with("/") {
                        s.push('/');
                    }
                    s.push_str("pgn");
                    Cow::Owned(s)
                };
                info!("Processing archive: {}", archive);
                let pgn = match self.download_pgn(archive.as_ref()) {
                    Ok(pgn) => pgn,
                    Err(e) => {
                        error!("downloading: {}", e);
                        continue;
                    }
                };
                if let Err(e) = fs::write(user_folder.join(format!("{}.pgn", i)), pgn.as_bytes()) {
                    error!("Failed to cache in config dir: {}", e);
                }
                if let Err(e) = db.add_multigame_pgn(pgn.as_bytes(), user.to_string()) {
                    error!("Failed to add to opening tree: {}", e);
                }
            }
        }
        db
    }

    pub fn get_user_archives(&self, user: &str) -> anyhow::Result<Vec<String>> {
        let url = format!("https://api.chess.com/pub/player/{}/games/archives", user);
        let resp = self.client.get(url).send()?.json::<Archives>()?;
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
