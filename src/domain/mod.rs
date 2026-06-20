pub mod item;
pub mod r#match;

use serde::Deserialize;

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
    Dynamic,
}
