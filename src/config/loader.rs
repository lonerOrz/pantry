use crate::config::parser::Config;
use std::path::PathBuf;

pub struct ConfigLoader {
    cache_dir: PathBuf,
}

impl ConfigLoader {
    pub fn new() -> Self {
        let cache_dir = dirs::cache_dir()
            .unwrap_or_else(|| std::env::current_dir().unwrap())
            .join("pantry");

        std::fs::create_dir_all(&cache_dir).expect("Failed to create cache directory");

        ConfigLoader { cache_dir }
    }

    pub fn load(&self, path: &str) -> Result<Config, ConfigError> {
        let content = std::fs::read_to_string(path).map_err(|e| ConfigError::IoError(e))?;

        toml::from_str(&content).map_err(|e| ConfigError::ParseError(e))
    }

    pub fn load_with_cache(&self, path: &str) -> Result<Config, ConfigError> {
        // 简单实现，后续可以添加缓存逻辑
        self.load(path)
    }
}

#[derive(Debug)]
pub enum ConfigError {
    IoError(std::io::Error),
    ParseError(toml::de::Error),
    NotFound,
}
