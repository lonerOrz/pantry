use serde::{Deserialize, Deserializer};
use std::collections::HashMap;

use crate::domain::{DisplayMode, SourceMode};

#[derive(Debug, Clone, Deserialize, PartialEq)]
#[serde(deny_unknown_fields)]
pub struct Category {
    pub display: Option<DisplayMode>,
    pub source: Option<SourceMode>,
    #[serde(default)]
    pub entries: HashMap<String, String>,
}

#[derive(Debug)]
pub struct Config {
    pub display: DisplayMode,
    pub source: SourceMode,
    pub categories: HashMap<String, Category>,
}

#[derive(Deserialize)]
struct RawConfig {
    pub display: Option<DisplayMode>,
    pub source: Option<SourceMode>,
    #[serde(flatten)]
    pub categories: HashMap<String, toml::Value>,
}

impl<'de> Deserialize<'de> for Config {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: Deserializer<'de>,
    {
        let raw = RawConfig::deserialize(deserializer)?;

        let display = raw.display.unwrap_or_default();
        let source = raw.source.unwrap_or_default();

        let mut categories = HashMap::new();
        for (name, val) in raw.categories {
            let category = Category::deserialize(val)
                .map_err(|e| serde::de::Error::custom(format!("In category [{}]: {}", name, e)))?;
            categories.insert(name, category);
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

    #[test]
    fn deny_unknown_fields_reports_typo() {
        let toml_str = r#"
display = "picture"

[favorites]
displey = "picture"
"#;
        let err = toml::from_str::<Config>(toml_str).unwrap_err();
        let msg = err.to_string();
        assert!(msg.contains("favorites"), "should mention category: {msg}");
        assert!(msg.contains("displey"), "should mention typo: {msg}");
    }

    #[test]
    fn deny_unknown_source_value() {
        let toml_str = r#"
display = "text"

[shell]
source = "dynamik"
"#;
        let err = toml::from_str::<Config>(toml_str).unwrap_err();
        let msg = err.to_string();
        assert!(msg.contains("shell"), "should mention category: {msg}");
    }
}
