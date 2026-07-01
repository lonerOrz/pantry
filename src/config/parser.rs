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
                    .get("entries")
                    .and_then(|v| v.as_table())
                    .map(|t| {
                        t.iter()
                            .filter_map(|(k, v)| v.as_str().map(|s| (k.clone(), s.to_string())))
                            .collect()
                    })
                    .unwrap_or_default();

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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parse_picture_config_with_entries_subtable() {
        let toml_str = r#"
display = "picture"

[favorites]
display = "picture"

[favorites.entries]
"cat" = "~/Pictures/wallpapers/cat.png"

[logos]
display = "picture"

[logos.entries]
"logos" = "~/Pictures/logos/"
"#;
        let config: Config = toml::from_str(toml_str).unwrap();
        assert_eq!(config.display, DisplayMode::Picture);
        assert_eq!(config.categories.len(), 2);

        let fav = config.categories.get("favorites").unwrap();
        assert_eq!(fav.display, Some(DisplayMode::Picture));
        assert_eq!(fav.entries.len(), 1);
        assert_eq!(
            fav.entries.get("cat").unwrap(),
            "~/Pictures/wallpapers/cat.png"
        );

        let logos = config.categories.get("logos").unwrap();
        assert_eq!(logos.entries.get("logos").unwrap(), "~/Pictures/logos/");
    }

    #[test]
    fn parse_text_config_with_command_source() {
        let toml_str = r#"
display = "text"

[shell]
display = "text"
source = "command"

[shell.entries]
"history" = "cat ~/.bash_history | tail -20"
"#;
        let config: Config = toml::from_str(toml_str).unwrap();
        assert_eq!(config.display, DisplayMode::Text);
        let shell = config.categories.get("shell").unwrap();
        assert_eq!(shell.source, Some(SourceMode::Command));
        assert_eq!(
            shell.entries.get("history").unwrap(),
            "cat ~/.bash_history | tail -20"
        );
    }
}
