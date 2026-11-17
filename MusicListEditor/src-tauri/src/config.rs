use std::fs;
use std::path::PathBuf;

use tauri::{AppHandle, Manager};

use crate::dto::AppConfig;

pub fn config_path(app: &AppHandle) -> Option<PathBuf> {
    app.path()
        .app_data_dir()
        .ok()
        .map(|dir| dir.join("config.json"))
}

pub fn load(app: &AppHandle) -> AppConfig {
    let Some(path) = config_path(app) else {
        return AppConfig::default();
    };
    match fs::read_to_string(&path) {
        Ok(text) => serde_json::from_str::<AppConfig>(&text).unwrap_or_default(),
        Err(_) => AppConfig::default(),
    }
}

pub fn save(app: &AppHandle, config: &AppConfig) {
    let Some(path) = config_path(app) else {
        return;
    };
    if let Some(parent) = path.parent() {
        let _ = fs::create_dir_all(parent);
    }
    if let Ok(text) = serde_json::to_string_pretty(config) {
        let _ = fs::write(path, text);
    }
}
