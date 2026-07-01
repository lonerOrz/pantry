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

impl Item {
    /// Get the display text content
    pub fn display_text(&self) -> String {
        self.value.clone()
    }
}
