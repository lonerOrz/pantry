use crate::config::{DisplayMode, SourceMode};

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
    pub fn builder() -> ItemBuilder {
        ItemBuilder::new()
    }

    /// Check if this item is in picture mode
    pub fn is_picture_mode(&self) -> bool {
        matches!(self.display, DisplayMode::Picture)
    }

    /// Get the display text content
    pub fn display_text(&self) -> String {
        self.value.clone()
    }
}

#[derive(Default)]
pub struct ItemBuilder {
    title: Option<String>,
    value: Option<String>,
    category: Option<String>,
    display: Option<DisplayMode>,
    source: Option<SourceMode>,
    preview_template: Option<String>,
}

impl ItemBuilder {
    pub fn new() -> Self {
        ItemBuilder::default()
    }

    pub fn title(mut self, title: String) -> Self {
        self.title = Some(title);
        self
    }

    pub fn value(mut self, value: String) -> Self {
        self.value = Some(value);
        self
    }

    pub fn category(mut self, category: String) -> Self {
        self.category = Some(category);
        self
    }

    pub fn display(mut self, display: DisplayMode) -> Self {
        self.display = Some(display);
        self
    }

    pub fn source(mut self, source: SourceMode) -> Self {
        self.source = Some(source);
        self
    }

    pub fn preview_template(mut self, template: String) -> Self {
        self.preview_template = Some(template);
        self
    }

    pub fn build(self) -> Item {
        Item {
            title: self.title.unwrap_or_default(),
            value: self.value.unwrap_or_default(),
            category: self.category.unwrap_or_default(),
            display: self.display.unwrap_or(DisplayMode::Text),
            source: self.source.unwrap_or(SourceMode::Config),
            preview_template: self.preview_template,
        }
    }
}
