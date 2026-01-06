use directories::ProjectDirs;
use serde::{Deserialize, Serialize};
use std::fs;
use std::path::PathBuf;
use thiserror::Error;

use crate::search::FrecencyData;

#[derive(Error, Debug)]
pub enum ConfigError {
    #[error("Could not determine config directory")]
    NoConfigDir,
    #[error("IO error: {0}")]
    Io(#[from] std::io::Error),
    #[error("JSON error: {0}")]
    Json(#[from] serde_json::Error),
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct Association {
    pub id: String,
    pub id_key: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct Config {
    pub association: Option<Association>,
    #[serde(default)]
    pub frecency: FrecencyData,
}

impl Config {
    fn data_dir() -> Result<PathBuf, ConfigError> {
        ProjectDirs::from("", "", "kpick")
            .map(|p| p.data_dir().to_path_buf())
            .ok_or(ConfigError::NoConfigDir)
    }

    fn config_path() -> Result<PathBuf, ConfigError> {
        Ok(Self::data_dir()?.join("config.json"))
    }

    pub fn load() -> Result<Self, ConfigError> {
        let path = Self::config_path()?;
        if !path.exists() {
            return Ok(Self::default());
        }
        let contents = fs::read_to_string(&path)?;
        Ok(serde_json::from_str(&contents)?)
    }

    pub fn save(&self) -> Result<(), ConfigError> {
        let path = Self::config_path()?;
        if let Some(parent) = path.parent() {
            fs::create_dir_all(parent)?;
        }
        let contents = serde_json::to_string_pretty(self)?;
        fs::write(&path, contents)?;
        Ok(())
    }
}
