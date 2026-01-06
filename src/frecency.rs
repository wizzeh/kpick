use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::fs;
use std::path::PathBuf;
use std::time::{SystemTime, UNIX_EPOCH};

/// Frecency tracking data - stored separately from config
#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct FrecencyData {
    #[serde(default)]
    pub entries: HashMap<String, FrecencyEntry>,
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct FrecencyEntry {
    pub count: u32,
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

    pub fn record_use(&mut self, uuid: &str) {
        let now = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap_or_default()
            .as_secs();
        let entry = self.entries.entry(uuid.to_string()).or_default();
        entry.count += 1;
        entry.last_used = now;
    }

    pub fn score(&self, uuid: &str) -> f64 {
        let Some(entry) = self.entries.get(uuid) else {
            return 0.0;
        };

        let now = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap_or_default()
            .as_secs();
        let age_hours = (now.saturating_sub(entry.last_used)) as f64 / 3600.0;

        // Frecency: count * recency_decay
        // Recency decays by half every 24 hours
        let recency = 0.5_f64.powf(age_hours / 24.0);
        entry.count as f64 * recency
    }
}
