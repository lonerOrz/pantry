use crate::config::{Category, Config, DisplayMode, SourceMode};
use crate::domain::item::Item;
use std::path::PathBuf;

pub fn load_items(
    config_path: &str,
    category_filter: &Option<String>,
    display_arg: &Option<String>,
) -> Result<Vec<Item>, String> {
    let config = parse_config(config_path)?;
    let items = collect_items(&config, category_filter, display_arg);
    Ok(crate::services::item_service::process_items_for_display(
        items,
    ))
}

pub fn get_default_config_path() -> String {
    let config_dir = dirs::config_dir()
        .unwrap_or_else(|| PathBuf::from("."))
        .join("pantry");

    if let Err(e) = std::fs::create_dir_all(&config_dir) {
        eprintln!("Warning: Failed to create config directory: {}", e);
    }

    config_dir.join("config.toml").to_string_lossy().to_string()
}

fn parse_config(config_path: &str) -> Result<Config, String> {
    let content = std::fs::read_to_string(config_path)
        .map_err(|e| format!("Failed to read config file {}: {}", config_path, e))?;
    toml::from_str(&content)
        .map_err(|e| format!("Failed to parse config file {}: {}", config_path, e))
}

fn collect_items(
    config: &Config,
    category_filter: &Option<String>,
    display_arg: &Option<String>,
) -> Vec<Item> {
    let mut items = Vec::new();

    for (name, category) in &config.categories {
        if !matches_category(
            name,
            category_filter,
            category,
            &config.display,
            display_arg,
        ) {
            continue;
        }

        let effective_display =
            crate::config::resolve_display_mode(display_arg, &category.display, &config.display);
        let effective_source = category.source.clone().unwrap_or(config.source.clone());

        load_category_items(
            name,
            category,
            effective_display,
            effective_source,
            &mut items,
        );
    }

    items
}

fn matches_category(
    name: &str,
    filter: &Option<String>,
    category: &Category,
    global_display: &DisplayMode,
    display_arg: &Option<String>,
) -> bool {
    if let Some(f) = filter {
        return name == f;
    }
    display_arg.is_some() || category.display.as_ref().unwrap_or(global_display) == global_display
}

fn load_category_items(
    category_name: &str,
    category_config: &Category,
    effective_display: DisplayMode,
    effective_source: SourceMode,
    items: &mut Vec<Item>,
) {
    match effective_source {
        SourceMode::Config => {
            for (key, value) in &category_config.entries {
                items.push(Item {
                    title: key.clone(),
                    value: value.clone(),
                    category: category_name.to_string(),
                    display: effective_display.clone(),
                    source: effective_source.clone(),
                    preview_template: None,
                });
            }
        }
        SourceMode::Command => {
            for (key, cmd) in &category_config.entries {
                if let Ok(output) = execute_command(cmd) {
                    let lines: Vec<&str> = output.lines().collect();
                    for (idx, line) in lines.iter().enumerate() {
                        if !line.trim().is_empty() {
                            let title = if lines.len() == 1 {
                                key.clone()
                            } else {
                                format!("{} [{}]", key, idx + 1)
                            };

                            items.push(Item {
                                title,
                                value: line.trim().to_string(),
                                category: category_name.to_string(),
                                display: effective_display.clone(),
                                source: effective_source.clone(),
                                preview_template: None,
                            });
                        }
                    }
                }
            }
        }
        SourceMode::Dynamic => {
            for (list_cmd, preview_template) in &category_config.entries {
                if let Ok(dynamic_items) =
                    crate::services::expansion::ItemProcessor::process_dynamic_source(
                        list_cmd,
                        preview_template,
                    )
                {
                    items.extend(dynamic_items);
                }
            }
        }
    }
}

fn execute_command(command: &str) -> Result<String, Box<dyn std::error::Error>> {
    let output = std::process::Command::new("sh")
        .arg("-c")
        .arg(command)
        .output()?;

    if output.status.success() {
        Ok(String::from_utf8(output.stdout)?)
    } else {
        let error_msg = String::from_utf8(output.stderr)?;
        Err(format!("Command failed: {}", error_msg).into())
    }
}
