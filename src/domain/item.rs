use super::{DisplayMode, SourceMode};

#[derive(Debug, Clone, PartialEq)]
pub struct Item {
    pub title: String,
    pub value: String,
    pub category: String,
    pub display: DisplayMode,
    pub source: SourceMode,
    pub preview_template: Option<String>,
}

