use clap::Parser;
use gtk4::{Application, gio, prelude::*};
use std::cell::RefCell;
use std::rc::Rc;

use crate::app::{
    event_handlers,
    preview_manager::PreviewManager,
    ui_builder::{self, UiMode},
};
use crate::domain::DisplayMode;
use crate::services::preview::create_prod_preview_service;
use crate::ui::list::ListState;
use crate::window_state::WindowState;

#[derive(Debug, Parser)]
#[command(
    name = "pantry",
    about = "A generic selector for various types of entries"
)]
pub struct Args {
    #[arg(short = 'f', long, default_value_t = crate::services::loader::get_default_config_path())]
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
        let preview_manager = Rc::new(RefCell::new(PreviewManager::new(
            create_prod_preview_service(),
        )));

        let search_query: crate::ui::search::SearchState = Rc::new(RefCell::new(String::new()));

        let parsed_config = if matches!(self.input_mode, InputMode::Config) {
            Some(crate::services::loader::parse_config(&self.args.config))
        } else {
            None
        };

        let mode = match &self.input_mode {
            InputMode::Stdin => UiMode::Stdin,
            InputMode::Config => match parsed_config.as_ref().unwrap() {
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
            },
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
        preview_manager: &Rc<
            RefCell<
                PreviewManager<
                    crate::cache::CacheManager,
                    crate::services::preview::ShellExec,
                    crate::services::preview::GdkPixbufDecoder,
                >,
            >,
        >,
    ) {
        let category_filter = category_filter.clone();
        let display_arg = display_arg.clone();
        let list_state = list_state.clone();
        let preview_area_rc_opt_clone = preview_area_rc_opt.clone();
        let preview_manager_clone = preview_manager.clone();

        glib::spawn_future_local(async move {
            let load_result = gio::spawn_blocking(move || {
                let processed_items = crate::services::pipeline::run(
                    &config,
                    &category_filter,
                    &display_arg,
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
