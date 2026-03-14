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

    /// 判断此项目是否在图片模式下显示
    pub fn is_picture_mode(&self) -> bool {
        matches!(self.display, DisplayMode::Picture)
    }

    /// 获取显示的文本内容
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

/// 项目处理器：处理图片目录展开等逻辑
pub struct ItemProcessor;

impl ItemProcessor {
    /// 处理项目用于显示（展开目录等）
    pub fn process_for_display(item: &Item) -> Vec<Item> {
        if matches!(item.display, DisplayMode::Picture) {
            let expanded_path = crate::utils::expand_tilde(&item.value);
            let expanded_path_str = expanded_path.to_string_lossy().to_string();

            if crate::utils::is_path_directory(&expanded_path_str) {
                use walkdir::WalkDir;
                let mut paths = Vec::new();
                for entry in WalkDir::new(&expanded_path_str)
                    .follow_links(true)
                    .into_iter()
                    .flatten()
                {
                    let path = entry.path();
                    if path.is_file() {
                        let path_str = path.to_string_lossy();
                        paths.push(Item {
                            title: format!(
                                "{} ({})",
                                path.file_name().unwrap_or_default().to_string_lossy(),
                                item.title
                            ),
                            value: path_str.to_string(),
                            category: item.category.clone(),
                            display: item.display.clone(),
                            source: item.source.clone(),
                            preview_template: item.preview_template.clone(),
                        });
                    }
                }
                paths
            } else {
                vec![Item {
                    title: item.title.clone(),
                    value: expanded_path_str,
                    category: item.category.clone(),
                    display: item.display.clone(),
                    source: item.source.clone(),
                    preview_template: item.preview_template.clone(),
                }]
            }
        } else {
            vec![item.clone()]
        }
    }

    /// Process dynamic source - execute list command and create items
    pub fn process_dynamic_source(
        list_command: &str,
        preview_template: &str,
    ) -> Result<Vec<Item>, Box<dyn std::error::Error>> {
        let output = std::process::Command::new("sh")
            .arg("-c")
            .arg(list_command)
            .output()?;

        if !output.status.success() {
            return Err(format!(
                "List command failed: {}",
                String::from_utf8_lossy(&output.stderr)
            )
            .into());
        }

        let raw_stdout = String::from_utf8_lossy(&output.stdout);
        let sanitized_stdout = raw_stdout.replace('\0', "");

        let mut items = Vec::new();
        // Only set template if non-empty
        let template = preview_template
            .is_empty()
            .then(|| preview_template.to_string());

        for line in sanitized_stdout.lines() {
            let line = line.trim();
            if line.is_empty() {
                continue;
            }

            let parts: Vec<&str> = line.split('\t').collect();
            let (id, display_text) = if parts.len() >= 2 {
                (parts[0].trim(), parts[1].trim())
            } else {
                (line, line)
            };

            let sanitized_id = id.replace('\0', "");
            let sanitized_display_text = display_text.replace('\0', "");

            items.push(Item {
                title: sanitized_display_text,
                value: sanitized_id,
                category: "dynamic".to_string(),
                display: DisplayMode::Text,
                source: SourceMode::Dynamic,
                preview_template: template.clone(),
            });
        }

        Ok(items)
    }
}
