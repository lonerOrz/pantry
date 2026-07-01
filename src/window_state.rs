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
            width: crate::constants::DEFAULT_WINDOW_WIDTH,
            height: crate::constants::DEFAULT_WINDOW_HEIGHT,
            maximized: false,
        }
    }
}

impl WindowState {
    pub fn load() -> Self {
        if let Some(config_path) = Self::get_config_path()
            && let Ok(contents) = fs::read_to_string(config_path)
            && let Ok(mut state) = toml::from_str::<WindowState>(&contents)
        {
            state.width = state.width.max(crate::constants::MIN_WINDOW_WIDTH);
            state.height = state.height.max(crate::constants::MIN_WINDOW_HEIGHT);
            return state;
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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn clamps_dirty_config_to_minimum() {
        let dirty = r#"width = 10
height = 5
maximized = false
"#;
        let mut state: WindowState = toml::from_str(dirty).unwrap();
        state.width = state.width.max(crate::constants::MIN_WINDOW_WIDTH);
        state.height = state.height.max(crate::constants::MIN_WINDOW_HEIGHT);
        assert_eq!(state.width, crate::constants::MIN_WINDOW_WIDTH);
        assert_eq!(state.height, crate::constants::MIN_WINDOW_HEIGHT);
    }

    #[test]
    fn normal_values_pass_through() {
        let ok = r#"width = 1200
height = 800
maximized = true
"#;
        let mut state: WindowState = toml::from_str(ok).unwrap();
        state.width = state.width.max(crate::constants::MIN_WINDOW_WIDTH);
        state.height = state.height.max(crate::constants::MIN_WINDOW_HEIGHT);
        assert_eq!(state.width, 1200);
        assert_eq!(state.height, 800);
    }
}
