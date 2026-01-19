use clap::Parser;
use gtk4::{prelude::*, Application, ApplicationWindow, ListBox, ScrolledWindow};
use std::cell::RefCell;
use std::process::Command;
use std::rc::Rc;

use crate::app::{event_handlers::EventHandler, search_logic::SearchLogic, ui_builder::UiBuilder};
use crate::config::{Config, DisplayMode, SourceMode};
use crate::domain::item::Item;
use crate::ui::preview;
use crate::window_state::WindowState;

#[derive(Debug, Parser)]
#[command(
    name = "pantry",
    about = "A generic selector for various types of entries"
)]
pub struct Args {
    /// Configuration file path [default: ~/.config/pantry/config.toml]
    #[arg(short = 'f', long)]
    pub config: Option<String>,

    /// Specify the category to load (load all categories if not specified)
    #[arg(short = 'c', long = "category")]
    pub category: Option<String>,

    /// Display mode: text or picture
    #[arg(short = 'd', long = "display")]
    pub display: Option<String>,

    /// Preview display: text or image (now read from config file, this parameter is deprecated)
    #[arg(long = "preview", hide = true, default_value = "auto")]
    pub preview_mode: String,
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
        let input_mode = if !atty::is(atty::Stream::Stdin) {
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
                let (window, listbox, preview_area_rc_opt, search_label) =
                    UiBuilder::build_stdin_ui(&self.args, &self.window_state, app);

                let search_query: crate::ui::search::SearchState =
                    Rc::new(RefCell::new(String::new()));
                SearchLogic::setup_filter_func(&listbox, search_query.clone());

                EventHandler::setup_keyboard_controller(
                    &window,
                    &listbox,
                    search_query,
                    search_label,
                    &self.args,
                    preview_area_rc_opt.clone(),
                );

                window.present();
            }
            InputMode::Config => {
                let (window, listbox, preview_area_rc_opt, search_label) =
                    UiBuilder::build_config_ui(&self.args, &self.window_state, app);

                let search_query: crate::ui::search::SearchState =
                    Rc::new(RefCell::new(String::new()));
                SearchLogic::setup_filter_func(&listbox, search_query.clone());

                EventHandler::setup_keyboard_controller(
                    &window,
                    &listbox,
                    search_query,
                    search_label,
                    &self.args,
                    preview_area_rc_opt.clone(),
                );

                self.load_items_from_config(
                    &listbox,
                    self.args.config.as_ref().unwrap(),
                    &self.args.category,
                    &self.args.display,
                    preview_area_rc_opt.clone(),
                );

                window.present();
            }
        }
    }

    fn wrap_in_scroll(&self, child: &impl gtk4::prelude::IsA<gtk4::Widget>) -> ScrolledWindow {
        let scrolled = ScrolledWindow::new();
        scrolled.set_child(Some(child));
        scrolled.set_vexpand(true);
        scrolled
    }

    fn save_current_window_state(&self, window: &ApplicationWindow) {
        let maximized = window.is_maximized();
        let (width, height) = window.default_size();
        let state = WindowState {
            width,
            height,
            maximized,
        };
        state.save();
    }

    fn load_items_from_config(
        &self,
        listbox: &ListBox,
        config_path: &str,
        category_filter: &Option<String>,
        display_arg: &Option<String>,
        preview_area_rc_opt: Option<std::rc::Rc<std::cell::RefCell<preview::PreviewArea>>>,
    ) {
        let config_path = config_path.to_string();
        let category_filter = category_filter.clone();
        let display_arg = display_arg.clone();
        let listbox_weak = listbox.downgrade();
        let preview_area_rc_opt_clone = preview_area_rc_opt.clone();

        glib::spawn_future_local(async move {
            let content = match std::fs::read_to_string(&config_path) {
                Ok(c) => c,
                Err(e) => {
                    eprintln!("Failed to read config file {}: {}", config_path, e);
                    return;
                }
            };

            let config: Config = match toml::from_str(&content) {
                Ok(c) => c,
                Err(e) => {
                    eprintln!("Failed to parse config file {}: {}", config_path, e);
                    return;
                }
            };

            let mut items = Vec::new();

            // If a category is specified, load only items from that category, otherwise load only categories with the same display mode as the global default
            if let Some(ref category) = category_filter {
                if let Some(category_config) = config.categories.get(category) {
                    let effective_display = if let Some(display_str) = &display_arg {
                        match display_str.as_str() {
                            "picture" => DisplayMode::Picture,
                            "text" => DisplayMode::Text,
                            _ => category_config
                                .display
                                .clone()
                                .unwrap_or(config.display.clone()),
                        }
                    } else {
                        category_config
                            .display
                            .clone()
                            .unwrap_or(config.display.clone())
                    };
                    let effective_source = category_config
                        .source
                        .clone()
                        .unwrap_or(config.source.clone());

                    match effective_source {
                        SourceMode::Config => {
                            // Static mode: use entries from config file
                            for (key, value) in &category_config.entries {
                                items.push(Item {
                                    title: key.clone(),
                                    value: value.clone(),
                                    category: category.clone(),
                                    display: effective_display.clone(),
                                    source: effective_source.clone(),
                                });
                            }
                        }
                        SourceMode::Command => {
                            // Command mode: execute command and use its output
                            for (key, cmd) in &category_config.entries {
                                if let Ok(output) = execute_command(cmd) {
                                    let lines: Vec<&str> = output.lines().collect();
                                    for (idx, line) in lines.iter().enumerate() {
                                        if !line.trim().is_empty() {
                                            let title = if lines.len() == 1 {
                                                key.clone() // Single line output uses original key
                                            } else {
                                                format!("{} [{}]", key, idx + 1)
                                                // Multi-line adds index
                                            };

                                            items.push(Item {
                                                title,
                                                value: line.trim().to_string(),
                                                category: category.clone(),
                                                display: effective_display.clone(),
                                                source: effective_source.clone(),
                                            });
                                        }
                                    }
                                }
                            }
                        }
                        SourceMode::Dynamic => {
                            // Dynamic mode: execute list command to get entries, template for preview
                            for (list_cmd, preview_template) in &category_config.entries {
                                if let Ok(dynamic_items) = crate::domain::item::ItemProcessor::process_dynamic_source(
                                    list_cmd,
                                    preview_template,
                                ) {
                                    items.extend(dynamic_items);
                                }
                            }
                        }
                    }
                }
            } else {
                // Load items only from categories that match the global default display mode
                for (category_name, category_config) in &config.categories {
                    let effective_display = if let Some(display_str) = &display_arg {
                        match display_str.as_str() {
                            "picture" => DisplayMode::Picture,
                            "text" => DisplayMode::Text,
                            _ => category_config
                                .display
                                .clone()
                                .unwrap_or(config.display.clone()),
                        }
                    } else {
                        category_config
                            .display
                            .clone()
                            .unwrap_or(config.display.clone())
                    };
                    let effective_source = category_config
                        .source
                        .clone()
                        .unwrap_or(config.source.clone());

                    // 如果有命令行参数，加载所有 categories；否则只加载匹配全局模式的
                    if display_arg.is_some() || effective_display == config.display {
                        match effective_source {
                            SourceMode::Config => {
                                for (key, value) in &category_config.entries {
                                    items.push(Item {
                                        title: key.clone(),
                                        value: value.clone(),
                                        category: category_name.clone(),
                                        display: effective_display.clone(),
                                        source: effective_source.clone(),
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
                                                    category: category_name.clone(),
                                                    display: effective_display.clone(),
                                                    source: effective_source.clone(),
                                                });
                                            }
                                        }
                                    }
                                }
                            }
                            SourceMode::Dynamic => {
                                // Dynamic mode: execute list command to get entries, template for preview
                                for (list_cmd, preview_template) in &category_config.entries {
                                    if let Ok(dynamic_items) = crate::domain::item::ItemProcessor::process_dynamic_source(
                                        list_cmd,
                                        preview_template,
                                    ) {
                                        items.extend(dynamic_items);
                                    }
                                }
                            }
                        }
                    }
                }
            }

            let processed_items = crate::services::ItemService::process_items_for_display(items);

            let listbox_weak_clone = listbox_weak.clone();
            let preview_area_rc_opt_clone = preview_area_rc_opt_clone.clone();

            glib::idle_add_local(move || {
                if let Some(listbox_strong) = listbox_weak_clone.upgrade() {
                    crate::services::ItemService::add_items_to_listbox(
                        &listbox_strong,
                        &processed_items,
                    );

                    // After all items are added, select the first item and trigger preview
                    crate::services::ItemService::select_first_item(&listbox_strong);

                    // Trigger initial preview update
                    glib::timeout_add_local(
                        std::time::Duration::from_millis(
                            crate::constants::INITIAL_PREVIEW_DELAY_MS,
                        ),
                        {
                            let listbox_clone = listbox_strong.clone();
                            let preview_area_rc_opt_clone = preview_area_rc_opt_clone.clone();
                            move || {
                                crate::app::preview_manager::PreviewManager::update_preview(
                                    &listbox_clone,
                                    &preview_area_rc_opt_clone,
                                );
                                glib::ControlFlow::Break
                            }
                        },
                    );
                }
                glib::ControlFlow::Break
            });
        });
    }
}

pub fn get_default_config_path() -> String {
    let config_dir = dirs::config_dir()
        .unwrap_or_else(|| std::env::current_dir().unwrap())
        .join("pantry");

    if let Err(e) = std::fs::create_dir_all(&config_dir) {
        eprintln!("Warning: Failed to create config directory: {}", e);
    }

    config_dir.join("config.toml").to_string_lossy().to_string()
}

// Helper function to execute commands and return output
fn execute_command(command: &str) -> Result<String, Box<dyn std::error::Error>> {
    let output = Command::new("sh").arg("-c").arg(command).output()?;

    if output.status.success() {
        Ok(String::from_utf8(output.stdout)?)
    } else {
        let error_msg = String::from_utf8(output.stderr)?;
        Err(format!("Command failed: {}", error_msg).into())
    }
}

use std::sync::{Mutex, OnceLock};

static LAST_PREVIEW_UPDATE: OnceLock<Mutex<u128>> = OnceLock::new();
