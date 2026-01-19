use clap::Parser;
use gtk4::{
    prelude::*, Application, ApplicationWindow, Box as GtkBox, EventControllerKey, Label, ListBox,
    ListBoxRow, Orientation, Overlay, PropagationPhase, ScrolledWindow,
};
use std::cell::RefCell;
use std::io::{self, Read};
use std::process::Command;
use std::rc::Rc;

use crate::config::{
    get_config_display_mode, resolve_display_mode, Config, DisplayMode, SourceMode,
};
use crate::domain::item::Item;
use crate::ui::{list, preview, window};
use crate::window_state::WindowState;

#[derive(Debug, Parser)]
#[command(
    name = "pantry",
    about = "A generic selector for various types of entries"
)]
pub struct Args {
    /// Configuration file path [default: ~/.config/pantry/config.toml]
    #[arg(short = 'f', long)]
    config: Option<String>,

    /// Specify the category to load (load all categories if not specified)
    #[arg(short = 'c', long = "category")]
    category: Option<String>,

    /// Display mode: text or picture
    #[arg(short = 'd', long = "display")]
    display: Option<String>,

    /// Preview display: text or image (now read from config file, this parameter is deprecated)
    #[arg(long = "preview", hide = true, default_value = "auto")]
    preview_mode: String,
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
            InputMode::Stdin => self.build_stdin_ui(app),
            InputMode::Config => self.build_config_ui(app),
        }
    }

    fn build_stdin_ui(&self, app: &Application) {
        // 从 stdin 读取数据
        let mut stdin_data = String::new();
        io::stdin().read_to_string(&mut stdin_data).unwrap();

        let window = window::create_main_window(app, &self.args);
        window.set_default_size(self.window_state.width, self.window_state.height);

        if self.window_state.maximized {
            window.maximize();
        }

        // 根据命令行参数或默认值确定显示模式
        let display_mode = resolve_display_mode(&self.args.display, &None, &DisplayMode::Text);

        let (main_widget, listbox, preview_area_rc_opt) =
            if matches!(display_mode, DisplayMode::Picture) {
                let paned = gtk4::Paned::new(gtk4::Orientation::Horizontal);

                let listbox = list::create_listbox();
                let scrolled = self.wrap_in_scroll(&listbox);
                scrolled.set_hexpand(true);
                paned.set_start_child(Some(&scrolled));

                let preview_area = preview::PreviewArea::new();
                preview_area.container.set_hexpand(true);
                preview_area.container.set_vexpand(true);
                paned.set_end_child(Some(&preview_area.container));

                paned.set_resize_start_child(true);
                paned.set_shrink_start_child(false);
                paned.set_resize_end_child(true);
                paned.set_shrink_end_child(false);

                paned.set_position(360);

                let preview_area_rc = std::rc::Rc::new(std::cell::RefCell::new(preview_area));

                (
                    paned.upcast::<gtk4::Widget>(),
                    listbox,
                    Some(preview_area_rc),
                )
            } else {
                let layout = GtkBox::new(Orientation::Vertical, 0);
                let listbox = list::create_listbox();
                let scrolled = self.wrap_in_scroll(&listbox);
                layout.append(&scrolled);

                (layout.upcast::<gtk4::Widget>(), listbox, None)
            };

        let (overlay, search_label) = self.create_search_overlay(&main_widget);
        window.set_child(Some(&overlay));

        let search_query: SearchState = Rc::new(RefCell::new(String::new()));
        self.setup_filter_func(&listbox, search_query.clone());

        let _config_path = self.args.config.as_ref().unwrap(); // 在 stdin 模式下这会使用默认值
        self.setup_keyboard_controller(
            &window,
            &listbox,
            search_query,
            search_label,
            &self.args,
            preview_area_rc_opt.clone(),
        );

        // 从 stdin 数据创建条目
        for line in stdin_data.lines() {
            if !line.trim().is_empty() {
                let item = Item {
                    title: line.to_string(),
                    value: line.to_string(),
                    category: "stdin".to_string(),
                    display: display_mode.clone(), // 使用确定的显示模式
                    source: SourceMode::Config,
                };

                self.add_item_to_ui(&listbox, item);
            }
        }

        // 选择第一个项目
        if let Some(first_row) = listbox.row_at_index(0) {
            listbox.select_row(Some(&first_row));
            first_row.grab_focus();
        }

        // 如果是图片模式，设置预览更新
        if matches!(display_mode, DisplayMode::Picture) {
            let preview_area_rc_opt_clone = preview_area_rc_opt.clone();
            let listbox_clone = listbox.clone();
            listbox.connect_selected_rows_changed(move |_listbox| {
                let preview_area_rc_opt_inner = preview_area_rc_opt_clone.clone();
                let listbox_inner = listbox_clone.clone();
                glib::timeout_add_local(std::time::Duration::from_millis(50), move || {
                    update_preview(&listbox_inner, &preview_area_rc_opt_inner);
                    glib::ControlFlow::Break
                });
            });

            if let Some(paned_widget) = main_widget.downcast_ref::<gtk4::Paned>() {
                let window_clone = window.clone();
                let paned_widget_clone = paned_widget.clone();
                glib::timeout_add_local(std::time::Duration::from_millis(100), move || {
                    let allocation = window_clone.default_size();
                    let width = allocation.0;

                    let position = (width as f64 * 0.3) as i32;
                    paned_widget_clone.set_position(position);

                    glib::ControlFlow::Break
                });

                let paned_widget_clone2 = paned_widget.clone();
                window.connect_realize(move |win| {
                    let win_clone = win.clone();
                    let paned_widget_clone3 = paned_widget_clone2.clone();
                    glib::timeout_add_local(std::time::Duration::from_millis(10), move || {
                        let allocation = win_clone.default_size();
                        let width = allocation.0;

                        let position = (width as f64 * 0.3) as i32;
                        paned_widget_clone3.set_position(position);

                        glib::ControlFlow::Break
                    });
                });
            }
        }

        window.present();
    }

    fn build_config_ui(&self, app: &Application) {
        let window = window::create_main_window(app, &self.args);
        window.set_default_size(self.window_state.width, self.window_state.height);

        if self.window_state.maximized {
            window.maximize();
        }

        let config_path = self.args.config.as_ref().unwrap();

        let (main_widget, listbox, preview_area_rc_opt) = if matches!(
            get_config_display_mode(config_path, &self.args.category, &self.args.display),
            DisplayMode::Picture
        ) {
            let paned = gtk4::Paned::new(gtk4::Orientation::Horizontal);

            let listbox = list::create_listbox();
            let scrolled = self.wrap_in_scroll(&listbox);
            scrolled.set_hexpand(true);
            paned.set_start_child(Some(&scrolled));

            let preview_area = preview::PreviewArea::new();
            preview_area.container.set_hexpand(true);
            preview_area.container.set_vexpand(true);
            paned.set_end_child(Some(&preview_area.container));

            paned.set_resize_start_child(true);
            paned.set_shrink_start_child(false);
            paned.set_resize_end_child(true);
            paned.set_shrink_end_child(false);

            paned.set_position(360);

            let preview_area_rc = std::rc::Rc::new(std::cell::RefCell::new(preview_area));

            (
                paned.upcast::<gtk4::Widget>(),
                listbox,
                Some(preview_area_rc),
            )
        } else {
            let layout = GtkBox::new(Orientation::Vertical, 0);
            let listbox = list::create_listbox();
            let scrolled = self.wrap_in_scroll(&listbox);
            layout.append(&scrolled);

            (layout.upcast::<gtk4::Widget>(), listbox, None)
        };

        let (overlay, search_label) = self.create_search_overlay(&main_widget);
        window.set_child(Some(&overlay));

        let search_query: SearchState = Rc::new(RefCell::new(String::new()));
        self.setup_filter_func(&listbox, search_query.clone());

        self.setup_keyboard_controller(
            &window,
            &listbox,
            search_query,
            search_label,
            &self.args,
            preview_area_rc_opt.clone(),
        );

        self.load_items_from_config(
            &listbox,
            config_path,
            &self.args.category,
            &self.args.display,
            preview_area_rc_opt.clone(),
        );

        if matches!(
            get_config_display_mode(config_path, &self.args.category, &self.args.display),
            DisplayMode::Picture
        ) {
            let preview_area_rc_opt_clone = preview_area_rc_opt.clone();
            let listbox_clone = listbox.clone();
            listbox.connect_selected_rows_changed(move |_listbox| {
                let preview_area_rc_opt_inner = preview_area_rc_opt_clone.clone();
                let listbox_inner = listbox_clone.clone();
                glib::timeout_add_local(std::time::Duration::from_millis(50), move || {
                    update_preview(&listbox_inner, &preview_area_rc_opt_inner);
                    glib::ControlFlow::Break
                });
            });
        }

        window.present();
    }

    fn create_search_overlay(
        &self,
        child: &impl gtk4::prelude::IsA<gtk4::Widget>,
    ) -> (Overlay, Label) {
        let overlay = Overlay::new();
        overlay.set_child(Some(child));
        let label = Label::new(None);
        label.add_css_class("app-notification");
        label.add_css_class("hidden");
        label.set_halign(gtk4::Align::Center);
        label.set_valign(gtk4::Align::End);
        label.set_margin_bottom(30);
        overlay.add_overlay(&label);
        (overlay, label)
    }

    fn setup_filter_func(&self, listbox: &ListBox, query_state: SearchState) {
        listbox.set_filter_func(Box::new(move |row: &ListBoxRow| -> bool {
            let query = query_state.borrow();
            if query.is_empty() {
                return true;
            }
            if let Some(item_ptr) = unsafe { row.data::<Item>("item") } {
                let item = unsafe { &*item_ptr.as_ptr() };
                let query_lower = query.to_lowercase();
                let title_lower = item.title.to_lowercase();
                let value_lower = item.value.to_lowercase();
                title_lower == query_lower
                    || value_lower == query_lower
                    || title_lower.contains(&query_lower)
                    || value_lower.contains(&query_lower)
                    || fuzzy_match(&title_lower, &query_lower)
                    || fuzzy_match(&value_lower, &query_lower)
            } else {
                false
            }
        }));
    }

    fn setup_keyboard_controller(
        &self,
        window: &ApplicationWindow,
        listbox: &ListBox,
        query_state: SearchState,
        search_label: Label,
        args: &Args,
        preview_area_rc_opt: Option<std::rc::Rc<std::cell::RefCell<preview::PreviewArea>>>,
    ) {
        let controller = EventControllerKey::new();
        controller.set_propagation_phase(PropagationPhase::Capture);
        let listbox = listbox.clone();
        let search_label = search_label.clone();
        let config_path = args.config.as_ref().unwrap();
        let preview_enabled = matches!(
            get_config_display_mode(config_path, &args.category, &args.display),
            DisplayMode::Picture
        );
        let preview_area_rc = preview_area_rc_opt;

        controller.connect_key_pressed(move |_, keyval, _, _| {
            if keyval == gtk4::gdk::Key::Return || keyval == gtk4::gdk::Key::KP_Enter {
                handle_selection(&listbox);
                return glib::Propagation::Stop;
            }
            if keyval == gtk4::gdk::Key::Escape {
                clear_search(&query_state, &listbox, &search_label, &preview_area_rc);
                return glib::Propagation::Stop;
            }

            // If in picture mode, handle selection changes to update preview
            if preview_enabled
                && (keyval == gtk4::gdk::Key::Down
                    || keyval == gtk4::gdk::Key::Up
                    || keyval == gtk4::gdk::Key::Tab
                    || keyval == gtk4::gdk::Key::ISO_Left_Tab)
            {
                // Delay preview update, wait for selection update to complete
                let listbox_clone = listbox.clone();
                let preview_area_rc_clone = preview_area_rc.clone();
                glib::timeout_add_local(std::time::Duration::from_millis(10), move || {
                    update_preview(&listbox_clone, &preview_area_rc_clone);
                    glib::ControlFlow::Break
                });
            }

            handle_search_input(
                keyval,
                &query_state,
                &listbox,
                &search_label,
                &preview_area_rc,
            )
        });
        window.add_controller(controller);
    }

    fn wrap_in_scroll(&self, child: &impl gtk4::prelude::IsA<gtk4::Widget>) -> ScrolledWindow {
        let scrolled = ScrolledWindow::new();
        scrolled.set_child(Some(child));
        scrolled.set_vexpand(true);
        scrolled
    }

    fn handle_selection(&self, listbox: &ListBox) {
        if let Some(selected_row) = listbox.selected_row() {
            if let Some(item_ptr) = unsafe { selected_row.data::<Item>("item") } {
                let item = unsafe { &*item_ptr.as_ptr() };

                print!("{}", item.value);

                use std::io::{self, Write};
                let _ = io::stdout().flush();

                if let Some(window) = listbox.root().and_downcast::<ApplicationWindow>() {
                    self.save_current_window_state(&window);
                    window.close();
                }
            }
        }
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

    fn clear_search(
        &self,
        query_state: &SearchState,
        listbox: &ListBox,
        label: &Label,
        preview_area_rc_opt: &Option<std::rc::Rc<std::cell::RefCell<preview::PreviewArea>>>,
    ) {
        query_state.borrow_mut().clear();
        label.add_css_class("hidden");
        listbox.invalidate_filter();
        self.update_selection_after_filter(listbox, preview_area_rc_opt);
    }

    fn handle_search_input(
        &self,
        keyval: gtk4::gdk::Key,
        query_state: &SearchState,
        listbox: &ListBox,
        label: &Label,
        preview_area_rc_opt: &Option<std::rc::Rc<std::cell::RefCell<preview::PreviewArea>>>,
    ) -> glib::Propagation {
        let (should_invalidate, current_text) = {
            let mut query = query_state.borrow_mut();
            let mut updated = false;
            if keyval == gtk4::gdk::Key::BackSpace {
                query.pop();
                updated = true;
            } else if let Some(c) = keyval.to_unicode() {
                if c.is_alphanumeric() || matches!(c, '-' | '_' | '.' | '/' | '@') {
                    query.push(c);
                    updated = true;
                }
            }
            (updated, query.clone())
        };
        if should_invalidate {
            if current_text.is_empty() {
                label.add_css_class("hidden");
            } else {
                label.set_text(&format!("Search: {}", current_text));
                label.remove_css_class("hidden");
            }
            listbox.invalidate_filter();
            self.update_selection_after_filter(listbox, preview_area_rc_opt);
            return glib::Propagation::Stop;
        }
        glib::Propagation::Proceed
    }

    fn update_selection_after_filter(
        &self,
        listbox: &ListBox,
        preview_area_rc_opt: &Option<std::rc::Rc<std::cell::RefCell<preview::PreviewArea>>>,
    ) {
        let mut needs_reselect = true;
        if let Some(selected) = listbox.selected_row() {
            if selected.is_child_visible() {
                selected.grab_focus();
                needs_reselect = false;
            }
        }
        if needs_reselect {
            if let Some(row) = self.first_visible_row_after_filter(listbox) {
                listbox.select_row(Some(&row));
                row.grab_focus();
            } else {
                listbox.select_row(None::<&ListBoxRow>);
            }
        }
        // Trigger preview update
        update_preview(listbox, preview_area_rc_opt);
    }

    fn first_visible_row_after_filter(&self, listbox: &ListBox) -> Option<ListBoxRow> {
        let mut i = 0;
        while let Some(row) = listbox.row_at_index(i) {
            if row.is_child_visible() {
                return Some(row);
            }
            i += 1;
        }
        None
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
                        }
                    }
                }
            }

            use rayon::prelude::*;
            let all_paths: Vec<_> = items
                .par_iter()
                .flat_map(process_item_for_display)
                .collect();

            let items = all_paths;

            let items_clone = items.clone();
            let listbox_weak_clone = listbox_weak.clone();
            glib::idle_add_local(move || {
                if let Some(listbox_strong) = listbox_weak_clone.upgrade() {
                    for item in &items_clone {
                        let row = list::create_list_item(&item.title, &item.value);
                        unsafe {
                            row.set_data("item", item.clone());
                        }
                        listbox_strong.append(&row);
                    }

                    // After all items are added, select the first item and trigger preview
                    if let Some(first_row) = listbox_strong.row_at_index(0) {
                        listbox_strong.select_row(Some(&first_row));
                        first_row.grab_focus();

                        // Trigger initial preview
                        update_preview(&listbox_strong, &preview_area_rc_opt_clone);
                    }
                }
                glib::ControlFlow::Break
            });
        });
    }

    fn add_item_to_ui(&self, listbox: &ListBox, item: Item) {
        let row = list::create_list_item(&item.title, &item.value);
        unsafe {
            row.set_data("item", item);
        }
        listbox.append(&row);
    }
}

type SearchState = Rc<RefCell<String>>;

fn get_default_config_path() -> String {
    let config_dir = dirs::config_dir()
        .unwrap_or_else(|| std::env::current_dir().unwrap())
        .join("pantry");

    std::fs::create_dir_all(&config_dir).expect("Failed to create config directory");

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

fn process_item_for_display(item: &Item) -> Vec<Item> {
    if let DisplayMode::Picture = item.display {
        let expanded_path = crate::utils::expand_tilde(&item.value);
        let expanded_path_str = expanded_path.to_string_lossy().to_string();

        if crate::utils::is_path_directory(&expanded_path_str) {
            use walkdir::WalkDir;
            let mut paths = Vec::new();
            for entry in WalkDir::new(&expanded_path_str)
                .follow_links(true)
                .into_iter()
                .flatten()
            {
                let path = entry.path();
                if path.is_file() {
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
                    });
                }
            }
            paths
        } else {
            vec![Item {
                title: item.title.clone(),
                value: expanded_path_str,
                category: item.category.clone(),
                display: item.display.clone(),
                source: item.source.clone(),
            }]
        }
    } else {
        vec![item.clone()]
    }
}

// 独立函数，用于处理 UI 事件，避免生命周期问题
fn handle_selection(listbox: &ListBox) {
    if let Some(selected_row) = listbox.selected_row() {
        if let Some(item_ptr) = unsafe { selected_row.data::<Item>("item") } {
            let item = unsafe { &*item_ptr.as_ptr() };

            print!("{}", item.value);

            use std::io::{self, Write};
            let _ = io::stdout().flush();

            if let Some(window) = listbox.root().and_downcast::<ApplicationWindow>() {
                let _window_state = WindowState::load();
                let (width, height) = window.default_size();
                let state = WindowState {
                    width,
                    height,
                    maximized: window.is_maximized(),
                };
                state.save();
                window.close();
            }
        }
    }
}

fn clear_search(
    query_state: &SearchState,
    listbox: &ListBox,
    label: &Label,
    preview_area_rc_opt: &Option<std::rc::Rc<std::cell::RefCell<preview::PreviewArea>>>,
) {
    query_state.borrow_mut().clear();
    label.add_css_class("hidden");
    listbox.invalidate_filter();
    update_selection_after_filter(listbox, preview_area_rc_opt);
}

fn update_selection_after_filter(
    listbox: &ListBox,
    preview_area_rc_opt: &Option<std::rc::Rc<std::cell::RefCell<preview::PreviewArea>>>,
) {
    let mut needs_reselect = true;
    if let Some(selected) = listbox.selected_row() {
        if selected.is_child_visible() {
            selected.grab_focus();
            needs_reselect = false;
        }
    }
    if needs_reselect {
        if let Some(row) = first_visible_row_after_filter(listbox) {
            listbox.select_row(Some(&row));
            row.grab_focus();
        } else {
            listbox.select_row(None::<&ListBoxRow>);
        }
    }
    // Trigger preview update
    update_preview(listbox, preview_area_rc_opt);
}

fn first_visible_row_after_filter(listbox: &ListBox) -> Option<ListBoxRow> {
    let mut i = 0;
    while let Some(row) = listbox.row_at_index(i) {
        if row.is_child_visible() {
            return Some(row);
        }
        i += 1;
    }
    None
}

fn update_preview(
    listbox: &ListBox,
    preview_area_rc_opt: &Option<std::rc::Rc<std::cell::RefCell<preview::PreviewArea>>>,
) {
    use std::time::{SystemTime, UNIX_EPOCH};

    let now = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|d| d.as_millis())
        .unwrap_or(0);

    {
        let mut last_update = LAST_PREVIEW_UPDATE.lock().unwrap();
        if now - *last_update < 50 {
            return;
        }
        *last_update = now;
    }

    if let Some(preview_area_rc) = preview_area_rc_opt {
        if let Some(selected_row) = listbox.selected_row() {
            if let Some(item_ptr) = unsafe { selected_row.data::<Item>("item") } {
                let item = unsafe { &*item_ptr.as_ptr() };

                let preview_area = &*preview_area_rc.borrow();
                preview_area.update_with_content(item);
            }
        }
    }
}

fn handle_search_input(
    keyval: gtk4::gdk::Key,
    query_state: &SearchState,
    listbox: &ListBox,
    label: &Label,
    preview_area_rc_opt: &Option<std::rc::Rc<std::cell::RefCell<preview::PreviewArea>>>,
) -> glib::Propagation {
    let (should_invalidate, current_text) = {
        let mut query = query_state.borrow_mut();
        let mut updated = false;
        if keyval == gtk4::gdk::Key::BackSpace {
            query.pop();
            updated = true;
        } else if let Some(c) = keyval.to_unicode() {
            if c.is_alphanumeric() || matches!(c, '-' | '_' | '.' | '/' | '@') {
                query.push(c);
                updated = true;
            }
        }
        (updated, query.clone())
    };
    if should_invalidate {
        if current_text.is_empty() {
            label.add_css_class("hidden");
        } else {
            label.set_text(&format!("Search: {}", current_text));
            label.remove_css_class("hidden");
        }
        listbox.invalidate_filter();
        update_selection_after_filter(listbox, preview_area_rc_opt);
        return glib::Propagation::Stop;
    }
    glib::Propagation::Proceed
}

fn fuzzy_match(text: &str, pattern: &str) -> bool {
    let text_chars: Vec<char> = text.chars().collect();
    let pattern_chars: Vec<char> = pattern.chars().collect();
    let mut text_idx = 0;
    let mut pattern_idx = 0;
    while text_idx < text_chars.len() && pattern_idx < pattern_chars.len() {
        if text_chars[text_idx] == pattern_chars[pattern_idx] {
            pattern_idx += 1;
        }
        text_idx += 1;
    }
    pattern_idx == pattern_chars.len()
}

use std::sync::Mutex;

lazy_static::lazy_static! {
    static ref LAST_PREVIEW_UPDATE: Mutex<u128> = Mutex::new(0);
}
