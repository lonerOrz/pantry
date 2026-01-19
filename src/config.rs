use serde::{Deserialize, Deserializer};
use std::collections::HashMap;

#[derive(Debug, Deserialize, Clone, PartialEq, Default)]
#[serde(rename_all = "lowercase")]
pub enum DisplayMode {
    #[default]
    Text,
    Picture,
}

#[derive(Debug, Deserialize, Clone, PartialEq, Default)]
#[serde(rename_all = "lowercase")]
pub enum SourceMode {
    #[default]
    Config,
    Command,
}

#[derive(Debug, Clone)]
pub struct Category {
    pub display: Option<DisplayMode>,
    pub source: Option<SourceMode>,
    pub entries: HashMap<String, String>,
}

#[derive(Deserialize, Debug)]
struct TempCategory {
    #[serde(default)]
    display: Option<DisplayMode>,
    #[serde(default)]
    source: Option<SourceMode>,
    #[serde(default)]
    entries: HashMap<String, String>,
}

#[derive(Deserialize, Debug)]
struct TempConfig {
    #[serde(default)]
    display: DisplayMode,
    #[serde(default)]
    source: SourceMode,
    #[serde(flatten)]
    categories: HashMap<String, serde_json::Value>,
}

impl TempConfig {
    fn parse_categories(self) -> Result<Config, Box<dyn std::error::Error>> {
        let mut parsed_categories = HashMap::new();

        for (name, value) in self.categories {
            // 解析每个类别，尝试将其作为包含 entries 的对象
            let temp_category: TempCategory = serde_json::from_value(value)?;
            parsed_categories.insert(
                name,
                Category {
                    display: temp_category.display,
                    source: temp_category.source,
                    entries: temp_category.entries,
                },
            );
        }

        Ok(Config {
            display: self.display,
            source: self.source,
            categories: parsed_categories,
        })
    }
}

#[derive(Debug)]
pub struct Config {
    pub display: DisplayMode,
    pub source: SourceMode,
    pub categories: HashMap<String, Category>,
}

impl<'de> Deserialize<'de> for Config {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: Deserializer<'de>,
    {
        let temp = TempConfig::deserialize(deserializer)?;
        temp.parse_categories().map_err(serde::de::Error::custom)
    }
}
