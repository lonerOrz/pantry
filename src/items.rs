use crate::config::{DisplayMode, SourceMode};

#[derive(Debug, Clone)]
pub struct Item {
    pub title: String,
    pub value: String,
    pub category: String,
    pub display: DisplayMode,
    pub source: SourceMode,
}
