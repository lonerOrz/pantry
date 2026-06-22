use crate::config::Config;
use std::path::PathBuf;

pub fn parse_config(config_path: &str) -> Result<Config, String> {
    let content = std::fs::read_to_string(config_path)
        .map_err(|e| format!("Failed to read config file {}: {}", config_path, e))?;
    toml::from_str(&content)
        .map_err(|e| format!("Failed to parse config file {}: {}", config_path, e))
}

pub fn get_default_config_path() -> String {
    let config_dir = dirs::config_dir()
        .unwrap_or_else(|| PathBuf::from("."))
        .join("pantry");

    if let Err(e) = std::fs::create_dir_all(&config_dir) {
        eprintln!("Warning: Failed to create config directory: {}", e);
    }

    config_dir.join("config.toml").to_string_lossy().to_string()
}
