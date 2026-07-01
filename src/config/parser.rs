use serde::{Deserialize, Deserializer};
use std::collections::HashMap;

use crate::domain::{DisplayMode, SourceMode};

#[derive(Debug, Clone)]
pub struct Category {
    pub display: Option<DisplayMode>,
    pub source: Option<SourceMode>,
    pub entries: HashMap<String, String>,
}

#[derive(Debug)]
pub struct Config {
    pub display: DisplayMode,
    pub source: SourceMode,
    pub categories: HashMap<String, Category>,
}

fn parse_display_mode(s: &str) -> DisplayMode {
    match s {
        "picture" => DisplayMode::Picture,
        _ => DisplayMode::default(),
    }
}

fn parse_source_mode(s: &str) -> SourceMode {
    match s {
        "command" => SourceMode::Command,
        "dynamic" => SourceMode::Dynamic,
        _ => SourceMode::default(),
    }
}

impl<'de> Deserialize<'de> for Config {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: Deserializer<'de>,
    {
        let raw: toml::Value = Deserialize::deserialize(deserializer)?;

        let table = raw
            .as_table()
            .ok_or_else(|| serde::de::Error::custom("expected a TOML table"))?;

        let display = table
            .get("display")
            .and_then(|v| v.as_str())
            .map(parse_display_mode)
            .unwrap_or_default();

        let source = table
            .get("source")
            .and_then(|v| v.as_str())
            .map(parse_source_mode)
            .unwrap_or_default();

        let mut categories = HashMap::new();
        for (name, value) in table {
            if name == "display" || name == "source" {
                continue;
            }
            if let Some(cat_table) = value.as_table() {
                let cat_display = cat_table
                    .get("display")
                    .and_then(|v| v.as_str())
                    .map(parse_display_mode);

                let cat_source = cat_table
                    .get("source")
                    .and_then(|v| v.as_str())
                    .map(parse_source_mode);

                let entries: HashMap<String, String> = cat_table
                    .iter()
                    .filter(|(k, _)| *k != "display" && *k != "source")
                    .filter_map(|(k, v)| v.as_str().map(|s| (k.clone(), s.to_string())))
                    .collect();

                categories.insert(
                    name.clone(),
                    Category {
                        display: cat_display,
                        source: cat_source,
                        entries,
                    },
                );
            }
        }

        Ok(Config {
            display,
            source,
            categories,
        })
    }
}
