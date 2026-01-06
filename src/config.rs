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

/// Color scheme for the UI
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ColorScheme {
    /// Background color
    #[serde(default = "ColorScheme::default_background")]
    pub background: String,
    /// Lighter background for input box
    #[serde(default = "ColorScheme::default_background_light")]
    pub background_light: String,
    /// Selection highlight color
    #[serde(default = "ColorScheme::default_selection")]
    pub selection: String,
    /// Subtle foreground for placeholders/hints
    #[serde(default = "ColorScheme::default_foreground_subtle")]
    pub foreground_subtle: String,
    /// Default foreground color
    #[serde(default = "ColorScheme::default_foreground")]
    pub foreground: String,
    /// Bright foreground for selected items
    #[serde(default = "ColorScheme::default_foreground_bright")]
    pub foreground_bright: String,
    /// Error color
    #[serde(default = "ColorScheme::default_error")]
    pub error: String,
}

impl ColorScheme {
    fn default_background() -> String { "#1e1e1e".to_string() }
    fn default_background_light() -> String { "#2d2d2d".to_string() }
    fn default_selection() -> String { "#264f78".to_string() }
    fn default_foreground_subtle() -> String { "#6e6e6e".to_string() }
    fn default_foreground() -> String { "#cccccc".to_string() }
    fn default_foreground_bright() -> String { "#ffffff".to_string() }
    fn default_error() -> String { "#ff6b6b".to_string() }

    /// Parse a hex color string like "#RRGGBB" to (R, G, B)
    pub fn parse_hex(hex: &str) -> (u8, u8, u8) {
        let hex = hex.trim_start_matches('#');
        if hex.len() != 6 {
            return (128, 128, 128); // Fallback gray
        }
        let r = u8::from_str_radix(&hex[0..2], 16).unwrap_or(128);
        let g = u8::from_str_radix(&hex[2..4], 16).unwrap_or(128);
        let b = u8::from_str_radix(&hex[4..6], 16).unwrap_or(128);
        (r, g, b)
    }

    /// Get all colors as RGB tuples
    pub fn to_rgb(&self) -> ColorSchemeRgb {
        ColorSchemeRgb {
            background: Self::parse_hex(&self.background),
            background_light: Self::parse_hex(&self.background_light),
            selection: Self::parse_hex(&self.selection),
            foreground_subtle: Self::parse_hex(&self.foreground_subtle),
            foreground: Self::parse_hex(&self.foreground),
            foreground_bright: Self::parse_hex(&self.foreground_bright),
            error: Self::parse_hex(&self.error),
        }
    }
}

impl Default for ColorScheme {
    fn default() -> Self {
        Self {
            background: Self::default_background(),
            background_light: Self::default_background_light(),
            selection: Self::default_selection(),
            foreground_subtle: Self::default_foreground_subtle(),
            foreground: Self::default_foreground(),
            foreground_bright: Self::default_foreground_bright(),
            error: Self::default_error(),
        }
    }
}

/// Parsed RGB colors for rendering
#[derive(Debug, Clone, Copy)]
pub struct ColorSchemeRgb {
    pub background: (u8, u8, u8),
    pub background_light: (u8, u8, u8),
    pub selection: (u8, u8, u8),
    pub foreground_subtle: (u8, u8, u8),
    pub foreground: (u8, u8, u8),
    pub foreground_bright: (u8, u8, u8),
    pub error: (u8, u8, u8),
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct Config {
    #[serde(default)]
    pub frecency: FrecencyData,
    #[serde(default)]
    pub colors: ColorScheme,
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
