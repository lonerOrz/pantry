use crate::domain::DisplayMode;

/// Unified display mode resolution with priority: command line > category > global > default
pub fn resolve_display_mode(
    display_arg: &Option<String>,
    category_display: &Option<DisplayMode>,
    global_display: &DisplayMode,
) -> DisplayMode {
    if let Some(display_str) = display_arg {
        match display_str.as_str() {
            "picture" => return DisplayMode::Picture,
            "text" => return DisplayMode::Text,
            _ => {}
        }
    }

    if let Some(cat_display) = category_display {
        return cat_display.clone();
    }

    global_display.clone()
}

pub fn get_config_display_mode(
    config_path: &str,
    category_filter: &Option<String>,
    display_arg: &Option<String>,
) -> DisplayMode {
    if let Ok(content) = std::fs::read_to_string(config_path)
        && let Ok(config) = toml::from_str::<crate::config::parser::Config>(&content)
    {
        if let Some(category) = category_filter
            && let Some(category_config) = config.categories.get(category)
        {
            return resolve_display_mode(display_arg, &category_config.display, &config.display);
        }
        return resolve_display_mode(display_arg, &None, &config.display);
    }
    resolve_display_mode(display_arg, &None, &DisplayMode::Text)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn resolve_cli_overrides_all() {
        assert_eq!(
            resolve_display_mode(
                &Some("picture".into()),
                &Some(DisplayMode::Text),
                &DisplayMode::Text
            ),
            DisplayMode::Picture
        );
    }

    #[test]
    fn resolve_category_over_global() {
        assert_eq!(
            resolve_display_mode(&None, &Some(DisplayMode::Picture), &DisplayMode::Text),
            DisplayMode::Picture
        );
    }

    #[test]
    fn resolve_global_default() {
        assert_eq!(
            resolve_display_mode(&None, &None, &DisplayMode::Text),
            DisplayMode::Text
        );
    }

    #[test]
    fn resolve_invalid_cli_falls_through() {
        assert_eq!(
            resolve_display_mode(
                &Some("invalid".into()),
                &Some(DisplayMode::Picture),
                &DisplayMode::Text
            ),
            DisplayMode::Picture
        );
    }
}
