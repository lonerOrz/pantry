use crate::config::Mode;

#[derive(Debug, Clone)]
pub struct Item {
    pub title: String,
    pub value: String,
    pub category: String,
    pub mode: Mode,
}