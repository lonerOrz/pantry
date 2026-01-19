use crate::config::DisplayMode;

/// 统一处理显示模式的优先级：命令行 > 类别设置 > 全局设置 > 默认值
pub fn resolve_display_mode(
    display_arg: &Option<String>,
    category_display: &Option<DisplayMode>,
    global_display: &DisplayMode,
) -> DisplayMode {
    // 优先使用命令行参数
    if let Some(display_str) = display_arg {
        match display_str.as_str() {
            "picture" => return DisplayMode::Picture,
            "text" => return DisplayMode::Text,
            _ => {} // 如果不是有效的显示模式，则继续使用配置文件
        }
    }

    // 其次使用类别设置
    if let Some(cat_display) = category_display {
        return cat_display.clone();
    }

    // 然后使用全局设置
    global_display.clone()
}

pub fn get_config_display_mode(
    config_path: &str,
    category_filter: &Option<String>,
    display_arg: &Option<String>,
) -> DisplayMode {
    if let Ok(content) = std::fs::read_to_string(config_path) {
        if let Ok(config) = toml::from_str::<crate::config::parser::Config>(&content) {
            if let Some(category) = category_filter {
                if let Some(category_config) = config.categories.get(category) {
                    return resolve_display_mode(
                        display_arg,
                        &category_config.display,
                        &config.display,
                    );
                }
            }
            return resolve_display_mode(display_arg, &None, &config.display);
        }
    }
    resolve_display_mode(display_arg, &None, &DisplayMode::Text)
}
