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

/// Item processor: handles directory expansion for picture mode, etc.
pub struct ItemProcessor;

impl ItemProcessor {
    const MEDIA_EXTENSIONS: &[&str] = &[
        "png", "jpeg", "jpg", "gif", "webp", "bmp", "tiff", "tif", "mp4", "webm", "mkv", "avi",
        "mov", "wmv", "flv", "m4v",
    ];

    /// Process item for display (expand directories, etc.)
    pub fn process_for_display(item: &Item) -> Vec<Item> {
        if matches!(item.display, DisplayMode::Picture) {
            let expanded_path = crate::utils::expand_tilde(&item.value);
            let expanded_path_str = expanded_path.to_string_lossy().to_string();

            if crate::utils::is_path_directory(&expanded_path_str) {
                use walkdir::WalkDir;
                let mut paths = Vec::new();
                for entry in WalkDir::new(&expanded_path_str)
                    .max_depth(3)
                    .follow_links(true)
                    .into_iter()
                    .flatten()
                {
                    let path = entry.path();
                    if path.is_file()
                        && path
                            .extension()
                            .and_then(|s| s.to_str())
                            .map(|ext| {
                                Self::MEDIA_EXTENSIONS.contains(&ext.to_lowercase().as_str())
                            })
                            .unwrap_or(false)
                    {
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
        let child = std::process::Command::new("sh")
            .arg("-c")
            .arg(list_command)
            .stdin(std::process::Stdio::null())
            .stdout(std::process::Stdio::piped())
            .stderr(std::process::Stdio::piped())
            .spawn()
            .map_err(|e| format!("Failed to spawn dynamic source command: {}", e))?;

        let pid = child.id();
        let (tx, rx) = std::sync::mpsc::channel();

        std::thread::spawn(move || {
            let result = child.wait_with_output();
            let _ = tx.send(result);
        });

        let output = match rx.recv_timeout(std::time::Duration::from_secs(30)) {
            Ok(Ok(output)) => output,
            Ok(Err(e)) => {
                return Err(format!("Dynamic source command failed: {}", e).into());
            }
            Err(_) => {
                let _ = std::process::Command::new("kill")
                    .arg("-9")
                    .arg(pid.to_string())
                    .output();
                return Err("Dynamic source command timed out after 30 seconds".into());
            }
        };

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
