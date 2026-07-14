use anyhow::Result;
use serde::{Deserialize, Serialize};
use std::path::PathBuf;

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(default)]
pub struct Settings {
    pub theme: String,
    pub timezone: String,
    pub language: String,
    pub region: String,
    pub date_format: String,
    pub time_format: String,
    pub use_24h: bool,
    pub notifications: bool,
    pub onboarded: bool,
    pub vm_network_mode: String,
}

impl Default for Settings {
    fn default() -> Self {
        Settings {
            theme: "dark".to_string(),
            timezone: "America/Chicago".to_string(),
            language: "en".to_string(),
            region: "US".to_string(),
            date_format: "%Y-%m-%d".to_string(),
            time_format: "%H:%M:%S".to_string(),
            use_24h: true,
            notifications: true,
            onboarded: false,
            vm_network_mode: "isolated".to_string(),
        }
    }
}

fn settings_path() -> Result<PathBuf> {
    Ok(super::ensure_data_dir()?.join("settings.json"))
}

pub fn load_settings() -> Settings {
    let Ok(path) = settings_path() else {
        return Settings::default();
    };
    match std::fs::read_to_string(&path) {
        Ok(data) => serde_json::from_str(&data).unwrap_or_default(),
        Err(_) => Settings::default(),
    }
}

pub fn save_settings(settings: &Settings) -> Result<()> {
    let path = settings_path()?;
    let data = serde_json::to_string_pretty(settings)?;
    std::fs::write(path, data)?;
    Ok(())
}
