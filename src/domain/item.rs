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
    pub fn config(
        title: impl Into<String>,
        value: impl Into<String>,
        category: impl Into<String>,
        display: DisplayMode,
    ) -> Self {
        Self {
            title: title.into(),
            value: value.into(),
            category: category.into(),
            display,
            source: SourceMode::Config,
            preview_template: None,
        }
    }

    pub fn command(
        title: impl Into<String>,
        value: impl Into<String>,
        category: impl Into<String>,
        display: DisplayMode,
    ) -> Self {
        Self {
            title: title.into(),
            value: value.into(),
            category: category.into(),
            display,
            source: SourceMode::Command,
            preview_template: None,
        }
    }

    pub fn dynamic(
        title: impl Into<String>,
        value: impl Into<String>,
        preview_template: Option<String>,
    ) -> Self {
        Self {
            title: title.into(),
            value: value.into(),
            category: "dynamic".to_string(),
            display: DisplayMode::Text,
            source: SourceMode::Dynamic,
            preview_template,
        }
    }

    pub fn stdin(value: impl Into<String>, display: DisplayMode) -> Self {
        let val = value.into();
        Self {
            title: val.clone(),
            value: val,
            category: "stdin".to_string(),
            display,
            source: SourceMode::Config,
            preview_template: None,
        }
    }
}
