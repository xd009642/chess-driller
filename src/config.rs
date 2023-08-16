use serde::{Deserialize, Serialize};
use std::fs;
use std::path::Path;

#[derive(Debug, Default, Clone, Serialize, Deserialize)]
pub struct Config {
    /// Chess.com usernames for the user
    #[serde(rename = "chess.com")]
    chess_com: Vec<String>,
}

impl Config {
    pub fn load() -> anyhow::Result<Self> {
        let config_dir = dirs::config_dir().unwrap_or_default().join("chess-driller");

        if !config_dir.exists() {
            println!(
                "Config file doesn't exist creating one in {}",
                config_dir.display()
            );
            if let Err(e) = fs::create_dir_all(config_dir.join("data")) {
                eprintln!("Couldn't create config dir: {}", e);
                Ok(Self::default())
            } else {
                Ok(Self::create_default(&config_dir.join("config.json")))
            }
        } else {
            let config_file = config_dir.join("config.json");
            if let Ok(data) = fs::read(&config_file) {
                let config = serde_json::from_slice(&data)?;
                Ok(config)
            } else {
                Ok(Self::create_default(&config_file))
            }
        }
    }

    fn create_default(save_dir: &Path) -> Self {
        let res = Self::default();
        let save = serde_json::to_vec_pretty(&res).unwrap();
        if let Err(e) = fs::write(save_dir, &save) {
            eprintln!("Failed to write out config file: {}", e);
        }
        res
    }
}
