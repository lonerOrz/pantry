use crate::constants::{DYNAMIC_OUTPUT_MAX_BYTES, MAX_ITEMS};
use crate::domain::item::Item;
use crate::domain::{DisplayMode, SourceMode};
use crate::services::process::CommandExecutor;

const MEDIA_EXTENSIONS: &[&str] = &[
    "png", "jpeg", "jpg", "gif", "webp", "bmp", "tiff", "tif", "mp4", "webm", "mkv", "avi", "mov",
    "wmv", "flv", "m4v",
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
                        .map(|ext| MEDIA_EXTENSIONS.contains(&ext.to_lowercase().as_str()))
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
            paths.truncate(MAX_ITEMS);
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
    executor: &dyn CommandExecutor,
) -> Result<Vec<Item>, Box<dyn std::error::Error>> {
    let output = executor.execute_with_timeout("sh", &["-c", list_command], 30)?;

    if !output.success {
        return Err("List command failed".into());
    }

    let truncated = output.stdout.len() >= DYNAMIC_OUTPUT_MAX_BYTES;
    if truncated {
        eprintln!(
            "Warning: dynamic source output exceeded {} bytes, truncated",
            DYNAMIC_OUTPUT_MAX_BYTES
        );
    }

    let raw_stdout = String::from_utf8_lossy(&output.stdout);
    let sanitized_stdout = raw_stdout.replace('\0', "");

    let mut items = Vec::new();
    let template = (!preview_template.is_empty()).then(|| preview_template.to_string());

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

        if items.len() >= MAX_ITEMS {
            break;
        }
    }

    Ok(items)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::services::process::MockExec;

    #[test]
    fn dynamic_single_line() {
        let exec = MockExec::new().push_ok(true, b"item1\tValue 1\n".to_vec());
        let items = process_dynamic_source("echo test", "", &exec).unwrap();
        assert_eq!(items.len(), 1);
        assert_eq!(items[0].title, "Value 1");
        assert_eq!(items[0].value, "item1");
        assert_eq!(items[0].source, SourceMode::Dynamic);
    }

    #[test]
    fn dynamic_multi_line() {
        let exec = MockExec::new().push_ok(true, b"a\t1\nb\t2\nc\t3\n".to_vec());
        let items = process_dynamic_source("echo test", "", &exec).unwrap();
        assert_eq!(items.len(), 3);
        assert_eq!(items[0].title, "1");
        assert_eq!(items[1].title, "2");
        assert_eq!(items[2].title, "3");
    }

    #[test]
    fn dynamic_empty_lines_skipped() {
        let exec = MockExec::new().push_ok(true, b"\na\t1\n\nb\t2\n".to_vec());
        let items = process_dynamic_source("echo test", "", &exec).unwrap();
        assert_eq!(items.len(), 2);
    }

    #[test]
    fn dynamic_template_applied() {
        let exec = MockExec::new().push_ok(true, b"id1\tName\n".to_vec());
        let items = process_dynamic_source("echo test", "preview {}", &exec).unwrap();
        assert_eq!(items[0].preview_template.as_deref(), Some("preview {}"));
    }

    #[test]
    fn dynamic_command_failure() {
        let exec = MockExec::new().push_ok(false, Vec::new());
        let result = process_dynamic_source("false", "", &exec);
        assert!(result.is_err());
    }

    #[test]
    fn dynamic_no_tab_single_field() {
        let exec = MockExec::new().push_ok(true, b"hello world\n".to_vec());
        let items = process_dynamic_source("echo test", "", &exec).unwrap();
        assert_eq!(items.len(), 1);
        assert_eq!(items[0].title, "hello world");
        assert_eq!(items[0].value, "hello world");
    }
}
