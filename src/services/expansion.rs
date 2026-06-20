use crate::constants::{DYNAMIC_OUTPUT_MAX_BYTES, MAX_ITEMS};
use crate::domain::item::Item;
use crate::domain::{DisplayMode, SourceMode};

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
    ) -> Result<Vec<Item>, Box<dyn std::error::Error>> {
        use std::io::Read;

        let mut child = std::process::Command::new("sh")
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
            let mut stdout = child.stdout.take().expect("stdout was piped");
            let mut buf = Vec::with_capacity(8192);
            let mut chunk = [0u8; 8192];

            loop {
                match stdout.read(&mut chunk) {
                    Ok(0) => break,
                    Ok(n) => {
                        if buf.len() + n > DYNAMIC_OUTPUT_MAX_BYTES {
                            break;
                        }
                        buf.extend_from_slice(&chunk[..n]);
                    }
                    Err(_) => break,
                }
            }

            let status = child.wait();
            let _ = tx.send((status, buf));
        });

        let (status, stdout_buf) = match rx.recv_timeout(std::time::Duration::from_secs(30)) {
            Ok(result) => result,
            Err(_) => {
                let _ = std::process::Command::new("kill")
                    .arg("-9")
                    .arg(pid.to_string())
                    .output();
                return Err("Dynamic source command timed out after 30 seconds".into());
            }
        };

        match status {
            Ok(s) if !s.success() => {
                return Err("List command failed".into());
            }
            Err(e) => {
                return Err(format!("Dynamic source command failed: {}", e).into());
            }
            _ => {}
        }

        let truncated = stdout_buf.len() >= DYNAMIC_OUTPUT_MAX_BYTES;
        if truncated {
            eprintln!(
                "Warning: dynamic source output exceeded {} bytes, truncated",
                DYNAMIC_OUTPUT_MAX_BYTES
            );
        }

        let raw_stdout = String::from_utf8_lossy(&stdout_buf);
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
}
