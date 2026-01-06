use directories::ProjectDirs;
use serde::{Deserialize, Serialize};
use std::fs;
use std::path::PathBuf;
use thiserror::Error;

#[derive(Error, Debug)]
pub enum ConfigError {
    #[error("Could not determine config directory")]
    NoConfigDir,
    #[error("IO error: {0}")]
    Io(#[from] std::io::Error),
    #[error("TOML parse error: {0}")]
    Toml(#[from] toml::de::Error),
    #[error("TOML serialize error: {0}")]
    TomlSerialize(#[from] toml::ser::Error),
}

/// Top-level configuration
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(default)]
pub struct Config {
    /// Path to KeePass database
    pub database_path: Option<String>,
    /// Seconds before clipboard is cleared (0 = never)
    pub clipboard_timeout: u64,
    /// Milliseconds to show the input flash indicator
    pub flash_duration: u64,
    /// Window settings
    pub window: WindowConfig,
    /// Font settings
    pub font: FontConfig,
    /// Color scheme
    pub colors: ColorScheme,
}

/// Window configuration
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(default)]
pub struct WindowConfig {
    /// Password prompt settings
    pub password: PasswordWindowConfig,
    /// Picker settings
    pub picker: PickerWindowConfig,
}

impl Default for WindowConfig {
    fn default() -> Self {
        Self {
            password: PasswordWindowConfig::default(),
            picker: PickerWindowConfig::default(),
        }
    }
}

/// Password window configuration
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(default)]
pub struct PasswordWindowConfig {
    /// Width in pixels
    pub width: u32,
    /// Height in pixels
    pub height: u32,
    /// Maximum percentage of screen
    pub max_percent: u32,
}

impl Default for PasswordWindowConfig {
    fn default() -> Self {
        Self {
            width: 400,
            height: 172,
            max_percent: 40,
        }
    }
}

/// Picker window configuration
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(default)]
pub struct PickerWindowConfig {
    /// Width as percentage of screen
    pub width_percent: u32,
    /// Height as percentage of screen
    pub height_percent: u32,
    /// Maximum entries visible
    pub max_entries: usize,
}

impl Default for PickerWindowConfig {
    fn default() -> Self {
        Self {
            width_percent: 50,
            height_percent: 40,
            max_entries: 10,
        }
    }
}

/// Font configuration
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(default)]
pub struct FontConfig {
    /// Font family name
    pub family: String,
    /// Main font size in pixels
    pub size: f32,
    /// Hints font size in pixels
    pub hints_size: f32,
}

impl Default for FontConfig {
    fn default() -> Self {
        Self {
            family: "DejaVu Sans".to_string(),
            size: 18.0,
            hints_size: 14.0,
        }
    }
}

/// Color scheme for the UI
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(default)]
pub struct ColorScheme {
    pub background: String,
    pub background_light: String,
    pub selection: String,
    pub foreground: String,
    pub foreground_subtle: String,
    pub foreground_bright: String,
    pub error: String,
}

impl Default for ColorScheme {
    fn default() -> Self {
        Self {
            background: "#1e1e1e".to_string(),
            background_light: "#2d2d2d".to_string(),
            selection: "#264f78".to_string(),
            foreground: "#cccccc".to_string(),
            foreground_subtle: "#6e6e6e".to_string(),
            foreground_bright: "#ffffff".to_string(),
            error: "#ff6b6b".to_string(),
        }
    }
}

impl ColorScheme {
    /// Parse a hex color string like "#RRGGBB" to (R, G, B)
    pub fn parse_hex(hex: &str) -> (u8, u8, u8) {
        let hex = hex.trim_start_matches('#');
        if hex.len() != 6 {
            return (128, 128, 128);
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
            foreground: Self::parse_hex(&self.foreground),
            foreground_subtle: Self::parse_hex(&self.foreground_subtle),
            foreground_bright: Self::parse_hex(&self.foreground_bright),
            error: Self::parse_hex(&self.error),
        }
    }
}

/// Parsed RGB colors for rendering
#[derive(Debug, Clone, Copy)]
pub struct ColorSchemeRgb {
    pub background: (u8, u8, u8),
    pub background_light: (u8, u8, u8),
    pub selection: (u8, u8, u8),
    pub foreground: (u8, u8, u8),
    pub foreground_subtle: (u8, u8, u8),
    pub foreground_bright: (u8, u8, u8),
    pub error: (u8, u8, u8),
}

impl Default for Config {
    fn default() -> Self {
        Self {
            database_path: None,
            clipboard_timeout: 10,
            flash_duration: 150,
            window: WindowConfig::default(),
            font: FontConfig::default(),
            colors: ColorScheme::default(),
        }
    }
}

impl Config {
    pub fn data_dir() -> Result<PathBuf, ConfigError> {
        ProjectDirs::from("", "", "kpick")
            .map(|p| p.data_dir().to_path_buf())
            .ok_or(ConfigError::NoConfigDir)
    }

    fn config_path() -> Result<PathBuf, ConfigError> {
        Ok(Self::data_dir()?.join("config.toml"))
    }

    pub fn load() -> Result<Self, ConfigError> {
        let path = Self::config_path()?;
        if !path.exists() {
            return Ok(Self::default());
        }
        let contents = fs::read_to_string(&path)?;
        Ok(toml::from_str(&contents)?)
    }

    pub fn save(&self) -> Result<(), ConfigError> {
        let path = Self::config_path()?;
        if let Some(parent) = path.parent() {
            fs::create_dir_all(parent)?;
        }
        let contents = toml::to_string_pretty(self)?;
        fs::write(&path, contents)?;
        Ok(())
    }

    /// Expand ~ in database path to home directory
    pub fn expand_database_path(&self) -> Option<PathBuf> {
        self.database_path.as_ref().map(|p| {
            if p.starts_with("~/") {
                if let Some(home) = dirs::home_dir() {
                    return home.join(&p[2..]);
                }
            }
            PathBuf::from(p)
        })
    }
}
