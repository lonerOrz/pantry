use clap::Parser;
use gtk4::{Application, gio, prelude::*};
use std::cell::RefCell;
use std::path::PathBuf;
use std::rc::Rc;

use crate::app::{
    event_handlers,
    preview_manager::PreviewManager,
    ui_builder::{self, UiMode},
};
use crate::domain::DisplayMode;
use crate::services::preview::create_prod_preview_service;
use crate::services::process::ShellExec;
use crate::ui::list::ListState;
use crate::window_state::WindowState;

fn parse_config(config_path: &str) -> Result<crate::config::Config, String> {
    let content = std::fs::read_to_string(config_path)
        .map_err(|e| format!("Failed to read config file {}: {}", config_path, e))?;
    toml::from_str(&content)
        .map_err(|e| format!("Failed to parse config file {}: {}", config_path, e))
}

fn get_default_config_path() -> String {
    let config_dir = dirs::config_dir()
        .unwrap_or_else(|| PathBuf::from("."))
        .join("pantry");

    if let Err(e) = std::fs::create_dir_all(&config_dir) {
        eprintln!("Warning: Failed to create config directory: {}", e);
    }

    config_dir.join("config.toml").to_string_lossy().to_string()
}

#[derive(Debug, Parser)]
#[command(
    name = "pantry",
    about = "A generic selector for various types of entries"
)]
pub struct Args {
    #[arg(short = 'f', long, default_value_t = get_default_config_path())]
    pub config: String,

    #[arg(short = 'c', long = "category")]
    pub category: Option<String>,

    #[arg(short = 'd', long = "display")]
    pub display: Option<String>,

    #[arg(short = 'm', long = "multi", help = "Enable multi-selection mode")]
    pub multi: bool,
}

pub struct PantryApp {
    args: Args,
    is_stdin: bool,
    window_state: WindowState,
}

impl PantryApp {
    pub fn new() -> Self {
        let args = Args::parse();
        let is_stdin = is_stdin_piped_or_redirected();
        let window_state = WindowState::load();

        PantryApp {
            args,
            is_stdin,
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
        use crate::app::preview_manager::PreviewUpdater;

        let raw_manager = PreviewManager::new(create_prod_preview_service());
        let preview_manager: Rc<RefCell<dyn PreviewUpdater>> = Rc::new(RefCell::new(raw_manager));

        let search_query: crate::ui::search::SearchState = Rc::new(RefCell::new(String::new()));

        let parsed_config = if !self.is_stdin {
            Some(parse_config(&self.args.config))
        } else {
            None
        };

        let mode = if self.is_stdin {
            UiMode::Stdin
        } else {
            match parsed_config.as_ref().unwrap() {
                Ok(config) => {
                    let display_mode = crate::config::get_config_display_mode(
                        config,
                        &self.args.category,
                        &self.args.display,
                    );
                    UiMode::Config { display_mode }
                }
                Err(e) => {
                    eprintln!("{}", e);
                    UiMode::Config {
                        display_mode: DisplayMode::Text,
                    }
                }
            }
        };

        let (window, list_state, preview_area_rc_opt, search_entry) = ui_builder::build_ui(
            &self.window_state,
            app,
            search_query.clone(),
            mode,
            &preview_manager,
        );

        event_handlers::setup_keyboard_controller(
            &window,
            &list_state,
            &search_entry,
            self.args.multi,
        );

        if let Some(Ok(config)) = parsed_config {
            self.load_items_from_config(
                &list_state,
                config,
                &self.args.category,
                &self.args.display,
                preview_area_rc_opt.clone(),
                &preview_manager,
            );
        }

        window.present();
        search_entry.grab_focus();
    }

    fn load_items_from_config(
        &self,
        list_state: &ListState,
        config: crate::config::Config,
        category_filter: &Option<String>,
        display_arg: &Option<String>,
        preview_area_rc_opt: Option<
            std::rc::Rc<std::cell::RefCell<crate::ui::preview::PreviewArea>>,
        >,
        preview_manager: &Rc<RefCell<dyn crate::app::preview_manager::PreviewUpdater>>,
    ) {
        let category_filter = category_filter.clone();
        let display_arg = display_arg.clone();
        let list_state = list_state.clone();
        let preview_area_rc_opt_clone = preview_area_rc_opt.clone();
        let preview_manager_clone = preview_manager.clone();

        glib::spawn_future_local(async move {
            let load_result = gio::spawn_blocking(move || {
                let executor = ShellExec;
                let processed_items = crate::services::pipeline::run(
                    &config,
                    &category_filter,
                    &display_arg,
                    &executor,
                );
                Ok::<Vec<crate::domain::item::Item>, String>(processed_items)
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

            list_state.append_items(&processed_items);
            list_state.select_first();

            glib::timeout_add_local(
                std::time::Duration::from_millis(crate::constants::INITIAL_PREVIEW_DELAY_MS),
                {
                    let list_state_clone = list_state.clone();
                    let preview_area_rc_opt_clone = preview_area_rc_opt_clone.clone();
                    move || {
                        preview_manager_clone
                            .borrow()
                            .update_preview(&list_state_clone, &preview_area_rc_opt_clone);
                        glib::ControlFlow::Break
                    }
                },
            );
        });
    }
}

fn is_stdin_piped_or_redirected() -> bool {
    use std::io::IsTerminal;
    !std::io::stdin().is_terminal()
}
