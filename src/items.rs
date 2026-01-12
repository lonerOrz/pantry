use crate::config::DisplayMode;

#[derive(Debug, Clone)]
pub struct Item {
    pub title: String,
    pub value: String,
    pub category: String,
    pub display: DisplayMode,
}