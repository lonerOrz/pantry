pub mod item;

use serde::Deserialize;
use std::str::FromStr;

#[derive(Debug, Deserialize, Clone, PartialEq, Default)]
#[serde(rename_all = "lowercase")]
pub enum DisplayMode {
    #[default]
    Text,
    Picture,
}

impl FromStr for DisplayMode {
    type Err = String;
    fn from_str(s: &str) -> Result<Self, Self::Err> {
        match s {
            "picture" => Ok(DisplayMode::Picture),
            "text" => Ok(DisplayMode::Text),
            _ => Err(format!("unknown display mode: {s}")),
        }
    }
}

#[derive(Debug, Deserialize, Clone, PartialEq, Default)]
#[serde(rename_all = "lowercase")]
pub enum SourceMode {
    #[default]
    Config,
    Command,
    Dynamic,
}

impl FromStr for SourceMode {
    type Err = String;
    fn from_str(s: &str) -> Result<Self, Self::Err> {
        match s {
            "command" => Ok(SourceMode::Command),
            "dynamic" => Ok(SourceMode::Dynamic),
            "config" => Ok(SourceMode::Config),
            _ => Err(format!("unknown source mode: {s}")),
        }
    }
}
