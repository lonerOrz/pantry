use clap::Parser;
use gtk4::{Application, gio, prelude::*};
use std::cell::RefCell;
use std::path::PathBuf;
use std::rc::Rc;

use crate::app::{event_handlers::EventHandler, ui_builder::UiBuilder};
use crate::config::{Config, DisplayMode, SourceMode};
use crate::domain::item::Item;
use crate::ui::list::ListState;
use crate::window_state::WindowState;

#[derive(Debug, Parser)]
#[command(
    name = "pantry",
    about = "A generic selector for various types of entries"
)]
pub struct Args {
    #[arg(short = 'f', long, default_value_t = crate::app::application::get_default_config_path())]
    pub config: String,

    #[arg(short = 'c', long = "category")]
    pub category: Option<String>,

    #[arg(short = 'd', long = "display")]
    pub display: Option<String>,
}

pub enum InputMode {
    Stdin,
    Config,
}

pub struct PantryApp {
    args: Args,
    input_mode: InputMode,
    window_state: WindowState,
}

impl PantryApp {
    pub fn new() -> Self {
        let args = Args::parse();
        let input_mode = if is_stdin_piped_or_redirected() {
            InputMode::Stdin
        } else {
            InputMode::Config
        };
        let window_state = WindowState::load();

        PantryApp {
            args,
            input_mode,
            window_state,
        }
    }

    pub fn run(self) {
        let app = Application::builder()
            .application_id("io.github.lonerorz.pantry")
            .build();
        app.connect_activate(move |app| self.build_ui(app));
        app.run_with_args(&Vec::<String>::new());
    }

    fn build_ui(&self, app: &Application) {
        match &self.input_mode {
            InputMode::Stdin => {
                let search_query: crate::ui::search::SearchState =
                    Rc::new(RefCell::new(String::new()));
                let (window, list_state, preview_area_rc_opt, search_entry) =
                    UiBuilder::build_stdin_ui(
                        &self.args,
                        &self.window_state,
                        app,
                        search_query.clone(),
                    );

                EventHandler::setup_keyboard_controller(
                    &window,
                    &list_state,
                    &search_entry,
                    preview_area_rc_opt.clone(),
                );

                window.present();
                search_entry.grab_focus();
            }
            InputMode::Config => {
                let search_query: crate::ui::search::SearchState =
                    Rc::new(RefCell::new(String::new()));
                let (window, list_state, preview_area_rc_opt, search_entry) =
                    UiBuilder::build_config_ui(
                        &self.args,
                        &self.window_state,
                        app,
                        search_query.clone(),
                    );

                EventHandler::setup_keyboard_controller(
                    &window,
                    &list_state,
                    &search_entry,
                    preview_area_rc_opt.clone(),
                );

                self.load_items_from_config(
                    &list_state,
                    &self.args.config,
                    &self.args.category,
                    &self.args.display,
                    preview_area_rc_opt.clone(),
                );

                window.present();
                search_entry.grab_focus();
            }
        }
    }

    fn load_items_from_config(
        &self,
        list_state: &ListState,
        config_path: &str,
        category_filter: &Option<String>,
        display_arg: &Option<String>,
        preview_area_rc_opt: Option<
            std::rc::Rc<std::cell::RefCell<crate::ui::preview::PreviewArea>>,
        >,
    ) {
        let config_path = config_path.to_string();
        let category_filter = category_filter.clone();
        let display_arg = display_arg.clone();
        let list_state = list_state.clone();
        let preview_area_rc_opt_clone = preview_area_rc_opt.clone();

        glib::spawn_future_local(async move {
            let load_result = gio::spawn_blocking(move || {
                load_items_from_config_sync(&config_path, &category_filter, &display_arg)
            })
            .await;

            let processed_items = match load_result {
                Ok(Ok(items)) => items,
                Ok(Err(e)) => {
                    eprintln!("{}", e);
                    return;
                }
                Err(e) => {
                    let panic_msg = if let Some(s) = e.downcast_ref::<&'static str>() {
                        *s
                    } else if let Some(s) = e.downcast_ref::<String>() {
                        s.as_str()
                    } else {
                        "Unknown panic payload"
                    };
                    eprintln!(
                        "Failed to load config items (thread panicked): {}",
                        panic_msg
                    );
                    return;
                }
            };

            crate::services::ItemService::add_items_to_list(&list_state, &processed_items);
            crate::services::ItemService::select_first_item(&list_state);

            glib::timeout_add_local(
                std::time::Duration::from_millis(crate::constants::INITIAL_PREVIEW_DELAY_MS),
                {
                    let list_state_clone = list_state.clone();
                    let preview_area_rc_opt_clone = preview_area_rc_opt_clone.clone();
                    move || {
                        crate::app::preview_manager::PreviewManager::update_preview(
                            &list_state_clone,
                            &preview_area_rc_opt_clone,
                        );
                        glib::ControlFlow::Break
                    }
                },
            );
        });
    }
}

fn load_items_from_config_sync(
    config_path: &str,
    category_filter: &Option<String>,
    display_arg: &Option<String>,
) -> Result<Vec<Item>, String> {
    let content = std::fs::read_to_string(config_path)
        .map_err(|e| format!("Failed to read config file {}: {}", config_path, e))?;

    let config: Config = toml::from_str(&content)
        .map_err(|e| format!("Failed to parse config file {}: {}", config_path, e))?;

    let mut items = Vec::new();

    for (category_name, category_config) in config.categories.iter().filter(|(name, cat_cfg)| {
        if let Some(filter) = category_filter {
            *name == filter
        } else {
            display_arg.is_some()
                || cat_cfg.display.as_ref().unwrap_or(&config.display) == &config.display
        }
    }) {
        let effective_display = crate::config::resolve_display_mode(
            display_arg,
            &category_config.display,
            &config.display,
        );
        let effective_source = category_config
            .source
            .clone()
            .unwrap_or(config.source.clone());

        load_items_from_category(
            category_name,
            category_config,
            effective_display,
            effective_source,
            &mut items,
        );
    }

    Ok(crate::services::ItemService::process_items_for_display(
        items,
    ))
}

fn load_items_from_category(
    category_name: &str,
    category_config: &crate::config::Category,
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

pub fn get_default_config_path() -> String {
    let config_dir = dirs::config_dir()
        .unwrap_or_else(|| PathBuf::from("."))
        .join("pantry");

    if let Err(e) = std::fs::create_dir_all(&config_dir) {
        eprintln!("Warning: Failed to create config directory: {}", e);
    }

    config_dir.join("config.toml").to_string_lossy().to_string()
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

fn is_stdin_piped_or_redirected() -> bool {
    #[cfg(unix)]
    {
        use std::os::unix::fs::FileTypeExt;
        if let Ok(metadata) = std::fs::metadata("/dev/stdin") {
            let file_type = metadata.file_type();
            file_type.is_fifo() || file_type.is_file()
        } else {
            false
        }
    }
    #[cfg(not(unix))]
    {
        !atty::is(atty::Stream::Stdin)
    }
}
