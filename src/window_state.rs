use serde::{Deserialize, Serialize};
use std::fs;
use std::path::PathBuf;

#[derive(Serialize, Deserialize, Debug)]
pub struct WindowState {
    pub width: i32,
    pub height: i32,
    pub maximized: bool,
}

impl Default for WindowState {
    fn default() -> Self {
        WindowState {
            width: 1200,
            height: 800,
            maximized: false,
        }
    }
}

impl WindowState {
    pub fn load() -> Self {
        if let Some(config_path) = Self::get_config_path() {
            if let Ok(contents) = fs::read_to_string(config_path) {
                if let Ok(state) = toml::from_str(&contents) {
                    return state;
                }
            }
        }

        WindowState::default()
    }

    pub fn save(&self) {
        if let Some(config_path) = Self::get_config_path() {
            if let Some(parent) = config_path.parent() {
                let _ = fs::create_dir_all(parent);
            }

            if let Ok(toml_string) = toml::to_string(self) {
                let _ = fs::write(config_path, toml_string);
            }
        }
    }

    fn get_config_path() -> Option<PathBuf> {
        dirs::config_dir().map(|config_dir| config_dir.join("pantry").join("window-state.toml"))
    }
}
