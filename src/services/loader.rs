use crate::config::{Category, Config};
use crate::constants::MAX_ITEMS;
use crate::domain::item::Item;
use crate::domain::{DisplayMode, SourceMode};
use crate::services::process::CommandExecutor;
use std::path::PathBuf;

pub fn load_items(
    config_path: &str,
    category_filter: &Option<String>,
    display_arg: &Option<String>,
) -> Result<Vec<Item>, String> {
    let config = parse_config(config_path)?;
    let executor = crate::services::process::ShellExec;
    let items = collect_items(&config, category_filter, display_arg, &executor);
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
    executor: &dyn CommandExecutor,
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
            executor,
        );
    }

    if items.len() > MAX_ITEMS {
        items.truncate(MAX_ITEMS);
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

pub(crate) fn load_category_items(
    category_name: &str,
    category_config: &Category,
    effective_display: DisplayMode,
    effective_source: SourceMode,
    items: &mut Vec<Item>,
    executor: &dyn CommandExecutor,
) {
    match effective_source {
        SourceMode::Config => {
            for (key, value) in &category_config.entries {
                if items.len() >= MAX_ITEMS {
                    return;
                }
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
                if let Ok(output) = execute_command(cmd, executor) {
                    let lines: Vec<&str> = output.lines().collect();
                    for (idx, line) in lines.iter().enumerate() {
                        if !line.trim().is_empty() {
                            if items.len() >= MAX_ITEMS {
                                return;
                            }
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
                        executor,
                    )
                {
                    items.extend(dynamic_items);
                }
            }
        }
    }
}

fn execute_command(
    command: &str,
    executor: &dyn CommandExecutor,
) -> Result<String, Box<dyn std::error::Error>> {
    let output = executor.execute("sh", &["-c", command])?;
    if output.success {
        Ok(String::from_utf8(output.stdout)?)
    } else {
        let error_msg = String::from_utf8_lossy(&output.stdout);
        Err(format!("Command failed: {}", error_msg).into())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::config::Category;
    use crate::services::process::MockExec;

    fn make_category(entries: Vec<(&str, &str)>) -> Category {
        Category {
            display: None,
            source: None,
            entries: entries.into_iter().map(|(k, v)| (k.into(), v.into())).collect(),
        }
    }

    #[test]
    fn config_source_populates_items() {
        let mut cat = make_category(vec![("a", "1"), ("b", "2")]);
        cat.source = Some(SourceMode::Config);
        let mut items = Vec::new();
        let exec = MockExec::new();
        load_category_items("test", &cat, DisplayMode::Text, SourceMode::Config, &mut items, &exec);
        assert_eq!(items.len(), 2);
        let titles: Vec<&str> = items.iter().map(|i| i.title.as_str()).collect();
        assert!(titles.contains(&"a"));
        assert!(titles.contains(&"b"));
        assert_eq!(items[0].value, if items[0].title == "a" { "1" } else { "2" });
    }

    #[test]
    fn command_source_uses_executor() {
        let mut cat = make_category(vec![("key", "echo hello")]);
        cat.source = Some(SourceMode::Command);
        let mut items = Vec::new();
        let exec = MockExec::new().push_ok(true, b"line1\nline2\n".to_vec());
        load_category_items("test", &cat, DisplayMode::Text, SourceMode::Command, &mut items, &exec);
        assert_eq!(items.len(), 2);
        assert_eq!(items[0].title, "key [1]");
        assert_eq!(items[0].value, "line1");
        assert_eq!(items[1].title, "key [2]");
        assert_eq!(items[1].value, "line2");
    }

    #[test]
    fn command_source_single_line_uses_key() {
        let mut cat = make_category(vec![("mykey", "echo one")]);
        cat.source = Some(SourceMode::Command);
        let mut items = Vec::new();
        let exec = MockExec::new().push_ok(true, b"only\n".to_vec());
        load_category_items("test", &cat, DisplayMode::Text, SourceMode::Command, &mut items, &exec);
        assert_eq!(items.len(), 1);
        assert_eq!(items[0].title, "mykey");
    }

    #[test]
    fn command_failure_skips_category() {
        let mut cat = make_category(vec![("k", "false")]);
        cat.source = Some(SourceMode::Command);
        let mut items = Vec::new();
        let exec = MockExec::new().push_ok(false, Vec::new());
        load_category_items("test", &cat, DisplayMode::Text, SourceMode::Command, &mut items, &exec);
        assert!(items.is_empty());
    }

    #[test]
    fn dynamic_source_uses_executor() {
        let mut cat = make_category(vec![("cmd", "tpl")]);
        cat.source = Some(SourceMode::Dynamic);
        let mut items = Vec::new();
        let exec = MockExec::new().push_ok(true, b"id\tName\n".to_vec());
        load_category_items("test", &cat, DisplayMode::Text, SourceMode::Dynamic, &mut items, &exec);
        assert_eq!(items.len(), 1);
        assert_eq!(items[0].title, "Name");
        assert_eq!(items[0].value, "id");
    }

    #[test]
    fn config_source_pushes_all_entries() {
        let keys: Vec<String> = (0..15).map(|i| format!("k{}", i)).collect();
        let entries: Vec<(&str, &str)> = keys.iter().map(|k| (k.as_str(), "v")).collect();
        let cat = Category {
            display: None,
            source: Some(SourceMode::Config),
            entries: entries.into_iter().map(|(k, v)| (k.into(), v.into())).collect(),
        };
        let mut items = Vec::new();
        let exec = MockExec::new();
        load_category_items("test", &cat, DisplayMode::Text, SourceMode::Config, &mut items, &exec);
        assert_eq!(items.len(), 15);
    }
}
