# TOML Config Implementation Plan

> **For Claude:** REQUIRED SUB-SKILL: Use superpowers:executing-plans to implement this plan task-by-task.

**Goal:** Replace JSON config with TOML and make all hardcoded values configurable.

**Architecture:** New `Config` struct with nested sub-structs for each section (WindowConfig, FontConfig, ColorScheme). Frecency data moves to separate JSON file. Font resolution searches system directories by family name.

**Tech Stack:** toml crate, serde, directories crate (existing)

---

### Task 1: Add TOML Dependency

**Files:**
- Modify: `Cargo.toml`

**Step 1: Add toml crate**

Add to `[dependencies]` section:
```toml
toml = "0.8"
```

**Step 2: Verify it compiles**

Run: `cargo build`
Expected: Successful compilation

**Step 3: Commit**

```bash
git add Cargo.toml Cargo.lock
git commit -m "deps: add toml crate for config parsing"
```

---

### Task 2: Create New Config Structs

**Files:**
- Modify: `src/config.rs`

**Step 1: Add new struct definitions**

Replace the entire `src/config.rs` with:

```rust
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
#[derive(Debug, Clone, Serialize, Deserialize, Default)]
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
```

**Step 2: Add dirs dependency for home expansion**

Add to `Cargo.toml` dependencies:
```toml
dirs = "5"
```

**Step 3: Verify it compiles**

Run: `cargo build`
Expected: Compilation errors about missing FrecencyData usage - this is expected, we'll fix in next task

---

### Task 3: Move Frecency to Separate File

**Files:**
- Create: `src/frecency.rs`
- Modify: `src/search.rs` (remove FrecencyData if defined there)
- Modify: `src/main.rs`

**Step 1: Check where FrecencyData is defined**

Run: `grep -r "struct FrecencyData" src/`

**Step 2: Create frecency module**

Create `src/frecency.rs`:

```rust
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::fs;
use std::path::PathBuf;

/// Frecency tracking data - stored separately from config
#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct FrecencyData {
    /// Map of entry UUID to usage data
    #[serde(default)]
    pub entries: HashMap<String, FrecencyEntry>,
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct FrecencyEntry {
    /// Number of times used
    pub count: u32,
    /// Unix timestamp of last use
    pub last_used: u64,
}

impl FrecencyData {
    fn data_path() -> Option<PathBuf> {
        directories::ProjectDirs::from("", "", "kpick")
            .map(|p| p.data_dir().join("frecency.json"))
    }

    pub fn load() -> Self {
        let Some(path) = Self::data_path() else {
            return Self::default();
        };
        if !path.exists() {
            return Self::default();
        }
        fs::read_to_string(&path)
            .ok()
            .and_then(|s| serde_json::from_str(&s).ok())
            .unwrap_or_default()
    }

    pub fn save(&self) -> Result<(), std::io::Error> {
        let Some(path) = Self::data_path() else {
            return Ok(());
        };
        if let Some(parent) = path.parent() {
            fs::create_dir_all(parent)?;
        }
        let contents = serde_json::to_string_pretty(self)?;
        fs::write(&path, contents)
    }

    /// Record a use of an entry
    pub fn record_use(&mut self, uuid: &str) {
        let now = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .map(|d| d.as_secs())
            .unwrap_or(0);

        let entry = self.entries.entry(uuid.to_string()).or_default();
        entry.count += 1;
        entry.last_used = now;
    }

    /// Calculate frecency score for an entry
    pub fn score(&self, uuid: &str) -> f64 {
        let Some(entry) = self.entries.get(uuid) else {
            return 0.0;
        };

        let now = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .map(|d| d.as_secs())
            .unwrap_or(0);

        let age_hours = (now.saturating_sub(entry.last_used)) as f64 / 3600.0;
        let recency = 1.0 / (1.0 + age_hours / 24.0);

        (entry.count as f64).sqrt() * recency
    }
}
```

**Step 3: Update src/main.rs module declarations**

Add at top of `src/main.rs`:
```rust
mod frecency;
```

**Step 4: Update search.rs to use new frecency module**

This depends on current search.rs structure - update imports to use `crate::frecency::FrecencyData`

**Step 5: Verify it compiles**

Run: `cargo build`

**Step 6: Commit**

```bash
git add -A
git commit -m "refactor: move frecency to separate module and file"
```

---

### Task 4: Implement Font Resolution

**Files:**
- Modify: `src/ui/wayland.rs`

**Step 1: Update font loading function**

Replace `load_system_font()` with:

```rust
/// Attempts to load a font by family name from system directories
fn load_font_by_family(family: &str) -> Font {
    let search_dirs = [
        "/usr/share/fonts",
        "/usr/local/share/fonts",
        "/run/current-system/sw/share/X11/fonts",
    ];

    // Also check ~/.local/share/fonts
    let home_fonts = dirs::home_dir().map(|h| h.join(".local/share/fonts"));

    let family_lower = family.to_lowercase();

    // Search for font matching family name
    for dir in search_dirs.iter().map(PathBuf::from).chain(home_fonts) {
        if let Some(font) = search_font_dir(&dir, &family_lower) {
            return font;
        }
    }

    // Fallback: try to find any TTF
    for dir in search_dirs.iter().map(PathBuf::from) {
        if let Some(font) = find_any_font(&dir) {
            return font;
        }
    }

    panic!("No fonts found. Please install DejaVu Sans or another TTF font.");
}

fn search_font_dir(dir: &Path, family_lower: &str) -> Option<Font> {
    let entries = fs::read_dir(dir).ok()?;

    for entry in entries.flatten() {
        let path = entry.path();

        if path.is_dir() {
            if let Some(font) = search_font_dir(&path, family_lower) {
                return Some(font);
            }
        } else if path.extension().map_or(false, |e| e == "ttf" || e == "otf") {
            let name = path.file_stem()?.to_string_lossy().to_lowercase();
            // Match if filename contains family name (handles "DejaVuSans", "DejaVu-Sans", etc.)
            let family_normalized = family_lower.replace(' ', "");
            if name.contains(&family_normalized) || name.contains(family_lower) {
                if let Ok(data) = fs::read(&path) {
                    if let Ok(font) = Font::from_bytes(data, FontSettings::default()) {
                        return Some(font);
                    }
                }
            }
        }
    }
    None
}

fn find_any_font(dir: &Path) -> Option<Font> {
    let entries = fs::read_dir(dir).ok()?;

    for entry in entries.flatten() {
        let path = entry.path();

        if path.is_dir() {
            if let Some(font) = find_any_font(&path) {
                return Some(font);
            }
        } else if path.extension().map_or(false, |e| e == "ttf") {
            if let Ok(data) = fs::read(&path) {
                if let Ok(font) = Font::from_bytes(data, FontSettings::default()) {
                    return Some(font);
                }
            }
        }
    }
    None
}
```

**Step 2: Add required imports**

Add to top of wayland.rs:
```rust
use std::path::Path;
```

**Step 3: Verify it compiles**

Run: `cargo build`

**Step 4: Commit**

```bash
git add src/ui/wayland.rs
git commit -m "feat: resolve fonts by family name"
```

---

### Task 5: Thread Config Through AppState

**Files:**
- Modify: `src/ui/wayland.rs`
- Modify: `src/ui/mod.rs`

**Step 1: Add config fields to AppState**

Add to AppState struct:
```rust
    // Config values
    flash_duration_ms: u64,
    font_size: f32,
    hints_font_size: f32,
    password_window: PasswordWindowConfig,
    picker_window: PickerWindowConfig,
```

**Step 2: Update AppState::new() to accept config**

Change signature to:
```rust
pub fn new(conn: &Connection, config: &Config, db_path: PathBuf) -> (Self, EventQueue<Self>)
```

Use config values:
- `config.colors.to_rgb()` for colors
- `config.flash_duration` for flash_duration_ms
- `config.font.size` and `config.font.hints_size` for font sizes
- `config.window.password` and `config.window.picker` for window configs
- `load_font_by_family(&config.font.family)` for font loading

**Step 3: Update size_for_mode() to use config**

Replace hardcoded values with self.password_window and self.picker_window fields.

**Step 4: Update draw_password_mode and draw_picker_mode**

Pass font sizes as parameters instead of using constants.

**Step 5: Update flash duration check**

Replace `Duration::from_millis(150)` with `Duration::from_millis(self.flash_duration_ms)`

**Step 6: Update main.rs**

Change AppState::new call to pass config reference.

**Step 7: Verify it compiles**

Run: `cargo build`

**Step 8: Commit**

```bash
git add -A
git commit -m "feat: thread config through AppState"
```

---

### Task 6: Use Config for Database Path and Clipboard Timeout

**Files:**
- Modify: `src/main.rs`

**Step 1: Update main() to use config database path**

```rust
// Get database path from config or command line
let db_path = config.expand_database_path()
    .unwrap_or_else(|| PathBuf::from("test.kdbx"));
```

**Step 2: Update clipboard timeout**

Replace hardcoded `10` with:
```rust
let timeout = config.clipboard_timeout;
// ...
if let Err(e) = copy_with_clear(value, timeout) {
```

**Step 3: Update message**

```rust
eprintln!("{} copied for: {} - {} (clears in {}s)", label, entry.title, entry.username, timeout);
```

**Step 4: Verify it compiles and runs**

Run: `cargo build && cargo run`

**Step 5: Commit**

```bash
git add src/main.rs
git commit -m "feat: use config for database path and clipboard timeout"
```

---

### Task 7: Clean Up and Test

**Step 1: Remove old JSON config handling**

Delete any remaining JSON-specific code.

**Step 2: Test with sample config**

Create `~/.local/share/kpick/config.toml`:
```toml
clipboard_timeout = 5

[colors]
background = "#000000"
```

**Step 3: Run and verify**

Run: `cargo run`
Verify: Background should be black, clipboard should clear in 5s.

**Step 4: Final commit**

```bash
git add -A
git commit -m "feat: complete TOML config implementation"
```

---

## Summary

After completing all tasks:
- Config is now TOML-based at `~/.local/share/kpick/config.toml`
- All previously hardcoded values are configurable
- Frecency data lives separately at `~/.local/share/kpick/frecency.json`
- Fonts are resolved by family name from system directories
- All fields have sensible defaults
