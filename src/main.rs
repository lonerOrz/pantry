use gtk4::{
    gdk, glib, prelude::*, Application, ApplicationWindow, EventControllerKey, Label, ListBox,
    ListBoxRow, Overlay, PropagationPhase, ScrolledWindow, Box as GtkBox, Orientation,
};
use std::cell::RefCell;
use std::rc::Rc;
use clap::Parser;

mod config;
mod items;
mod ui;
mod utils;
use config::{Config, Mode};
use items::Item;
use ui::{window, list, preview};

const APP_ID: &str = "eu.soliprem.pantry";

#[derive(Parser, Debug)]
#[command(name = "pantry", about = "A generic selector for various types of entries")]
struct Args {
    /// Configuration file path [default: ~/.config/pantry/config.toml]
    #[arg(short = 'f', long)]
    config: Option<String>,

    /// Specify the category to load (load all categories if not specified)
    #[arg(short = 'c', long = "category")]
    category: Option<String>,

    /// Preview mode: text or image (now read from config file, this parameter is deprecated)
    #[arg(long = "preview", hide = true, default_value = "auto")]
    preview_mode: String,  // "auto", "text", "image"
}

type SearchState = Rc<RefCell<String>>;

fn get_default_config_path() -> String {
    let config_dir = dirs::config_dir()
        .unwrap_or_else(|| std::env::current_dir().unwrap())
        .join("pantry");

    // Ensure the config directory exists
    std::fs::create_dir_all(&config_dir).expect("Failed to create config directory");

    config_dir.join("config.toml").to_string_lossy().to_string()
}

fn main() {
    let mut args = Args::parse();

    // Set default config path if not provided
    if args.config.is_none() {
        args.config = Some(get_default_config_path());
    }

    let app = Application::builder().application_id(APP_ID).build();
    app.connect_activate(move |app| build_ui(app, &args));
    app.run_with_args(&Vec::<String>::new());
}

fn build_ui(app: &Application, args: &Args) {
    let window = window::create_main_window(app, args);

    let config_path = args.config.as_ref().unwrap(); // Safe to unwrap since we set a default

    let (main_widget, listbox, preview_area_rc_opt) = if matches!(get_config_mode(config_path, &args.category), Mode::Picture) {
        let paned = gtk4::Paned::new(gtk4::Orientation::Horizontal);

        let listbox = list::create_listbox();
        let scrolled = wrap_in_scroll(&listbox);
        scrolled.set_hexpand(true);
        paned.set_start_child(Some(&scrolled));

        let preview_area = preview::PreviewArea::new();
        // Set expansion properties for preview area
        preview_area.container.set_hexpand(true);
        preview_area.container.set_vexpand(true);
        paned.set_end_child(Some(&preview_area.container));

        // Set Paned component properties to maintain fixed ratio
        paned.set_resize_start_child(false);  // List area doesn't adjust with window size
        paned.set_shrink_start_child(false);
        paned.set_resize_end_child(true);   // Preview area adjusts with window size
        paned.set_shrink_end_child(false);

        // Set initial position, e.g. list area takes 400px (at default window size)
        paned.set_position(400);

        // Wrap preview_area in Rc<RefCell>
        let preview_area_rc = std::rc::Rc::new(std::cell::RefCell::new(preview_area));

        (paned.upcast::<gtk4::Widget>(), listbox, Some(preview_area_rc))
    } else {
        let layout = GtkBox::new(Orientation::Vertical, 0);
        let listbox = list::create_listbox();
        let scrolled = wrap_in_scroll(&listbox);
        layout.append(&scrolled);

        (layout.upcast::<gtk4::Widget>(), listbox, None)
    };

    let (overlay, search_label) = create_search_overlay(&main_widget);
    window.set_child(Some(&overlay));

    let search_query: SearchState = Rc::new(RefCell::new(String::new()));
    setup_filter_func(&listbox, search_query.clone());

    setup_keyboard_controller(&window, &listbox, search_query, search_label, args, preview_area_rc_opt.clone());

    load_items_from_config(&listbox, config_path, &args.category, preview_area_rc_opt.clone());


    // Add ListBox selection change listener to update preview
    if matches!(get_config_mode(config_path, &args.category), Mode::Picture) {
        let preview_area_rc_opt_clone = preview_area_rc_opt.clone();
        let listbox_clone = listbox.clone();
        listbox.connect_selected_rows_changed(move |_listbox| {
            // Use glib::timeout_add_local to delay preview update, avoiding UI blocking
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

fn get_config_mode(config_path: &str, category_filter: &Option<String>) -> Mode {
    if let Ok(content) = std::fs::read_to_string(config_path) {
        if let Ok(config) = toml::from_str::<Config>(&content) {
            if let Some(category) = category_filter {
                if let Some(category_config) = config.categories.get(category) {
                    return category_config.mode.clone().unwrap_or(config.mode.clone());
                }
            }
            return config.mode;
        }
    }
    Mode::Text
}

// --- UI Components ---

fn wrap_in_scroll(child: &impl gtk4::prelude::IsA<gtk4::Widget>) -> ScrolledWindow {
    let scrolled = ScrolledWindow::new();
    scrolled.set_child(Some(child));
    scrolled.set_vexpand(true);
    scrolled
}

fn create_search_overlay(child: &impl gtk4::prelude::IsA<gtk4::Widget>) -> (Overlay, Label) {
    let overlay = Overlay::new();
    overlay.set_child(Some(child));
    let label = Label::new(None);
    label.add_css_class("app-notification");
    label.set_halign(gtk4::Align::Center);
    label.set_valign(gtk4::Align::End);
    label.set_margin_bottom(30);
    label.set_visible(false);
    overlay.add_overlay(&label);
    (overlay, label)
}

// --- Logic & Events ---

fn setup_filter_func(listbox: &ListBox, query_state: SearchState) {
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
            let matches = if title_lower == query_lower || value_lower == query_lower {
                true
            } else if title_lower.contains(&query_lower) || value_lower.contains(&query_lower) {
                true
            } else if fuzzy_match(&title_lower, &query_lower) || fuzzy_match(&value_lower, &query_lower) {
                true
            } else {
                false
            };
            matches
        } else {
            false
        }
    }));
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

fn setup_keyboard_controller(
    window: &ApplicationWindow,
    listbox: &ListBox,
    query_state: SearchState,
    search_label: Label,
    args: &Args,
    preview_area_rc_opt: Option<std::rc::Rc<std::cell::RefCell<preview::PreviewArea>>>, // Modify parameter type
) {
    let controller = EventControllerKey::new();
    controller.set_propagation_phase(PropagationPhase::Capture);
    let listbox = listbox.clone();
    let search_label = search_label.clone();
    let config_path = args.config.as_ref().unwrap(); // Safe to unwrap since we set a default
    let preview_enabled = matches!(get_config_mode(config_path, &args.category), Mode::Picture);
    // let preview_area_rc = preview_area_opt.map(|p| std::rc::Rc::new(std::cell::RefCell::new(p))); // 移除此行，因为现在直接接收 Rc 了
    let preview_area_rc = preview_area_rc_opt;

    controller.connect_key_pressed(move |_, keyval, _, _| {
        if keyval == gdk::Key::Return || keyval == gdk::Key::KP_Enter {
            handle_selection(&listbox);
            return glib::Propagation::Stop;
        }
        if keyval == gdk::Key::Escape {
            clear_search(&query_state, &listbox, &search_label, &preview_area_rc);
            return glib::Propagation::Stop;
        }

        // If in picture mode, handle selection changes to update preview
        if preview_enabled {
            if keyval == gdk::Key::Down || keyval == gdk::Key::Up ||
               keyval == gdk::Key::Tab || keyval == gdk::Key::ISO_Left_Tab {
                // Delay preview update, wait for selection update to complete
                let listbox_clone = listbox.clone();
                let preview_area_rc_clone = preview_area_rc.clone();
                glib::timeout_add_local(std::time::Duration::from_millis(10), move || {
                    update_preview(&listbox_clone, &preview_area_rc_clone);
                    glib::ControlFlow::Break
                });
            }
        }

        handle_search_input(keyval, &query_state, &listbox, &search_label, &preview_area_rc)
    });
    window.add_controller(controller);
}

// 用于防抖的全局变量
use std::sync::Mutex;
use std::time::{SystemTime, UNIX_EPOCH};

lazy_static::lazy_static! {
    static ref LAST_PREVIEW_UPDATE: Mutex<u128> = Mutex::new(0);
}

fn update_preview(listbox: &ListBox, preview_area_rc_opt: &Option<std::rc::Rc<std::cell::RefCell<preview::PreviewArea>>>) {
    // 防抖：如果上次更新时间距离现在不到50毫秒，则不更新
    let now = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|d| d.as_millis())
        .unwrap_or(0);

    {
        let mut last_update = LAST_PREVIEW_UPDATE.lock().unwrap();
        if now - *last_update < 50 {
            return; // 距离上次更新不足50毫秒，跳过本次更新
        }
        *last_update = now;
    }

    if let Some(preview_area_rc) = preview_area_rc_opt {
        if let Some(selected_row) = listbox.selected_row() {
            if let Some(item_ptr) = unsafe { selected_row.data::<Item>("item") } {
                let item = unsafe { &*item_ptr.as_ptr() };

                if matches!(item.mode, Mode::Picture) && utils::is_image_file(&item.value) {
                    let preview_area = &*preview_area_rc.borrow();
                    preview_area.update_with_image(item);
                } else {
                    let preview_area = &*preview_area_rc.borrow();
                    preview_area.clear();
                }
            }
        }
    }
}

fn clear_search(query_state: &SearchState, listbox: &ListBox, label: &Label, preview_area_rc_opt: &Option<std::rc::Rc<std::cell::RefCell<preview::PreviewArea>>>) {
    query_state.borrow_mut().clear();
    label.set_visible(false);
    listbox.invalidate_filter();
    update_selection_after_filter(listbox, preview_area_rc_opt);
}

fn handle_selection(listbox: &ListBox) {
    if let Some(selected_row) = listbox.selected_row() {
        if let Some(item_ptr) = unsafe { selected_row.data::<Item>("item") } {
            let item = unsafe { &*item_ptr.as_ptr() };

            // 直接输出值到标准输出
            print!("{}", item.value);

            // 确保立即刷新输出（对管道特别重要）
            use std::io::{self, Write};
            let _ = io::stdout().flush();

            // 退出程序
            if let Some(window) = listbox.root().and_downcast::<ApplicationWindow>() {
                window.close();
            }
        }
    }
}

// --- Input Handler with UI Feedback ---

fn handle_search_input(
    keyval: gdk::Key,
    query_state: &SearchState,
    listbox: &ListBox,
    label: &Label,
    preview_area_rc_opt: &Option<std::rc::Rc<std::cell::RefCell<preview::PreviewArea>>>, // 新增参数
) -> glib::Propagation {
    let (should_invalidate, current_text) = {
        let mut query = query_state.borrow_mut();
        let mut updated = false;
        if keyval == gdk::Key::BackSpace {
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
            label.set_visible(false);
        } else {
            label.set_text(&format!("Search: {}", current_text));
            label.set_visible(true);
        }
        listbox.invalidate_filter();
        update_selection_after_filter(listbox, preview_area_rc_opt); // 传递 preview_area_rc_opt
        return glib::Propagation::Stop;
    }
    glib::Propagation::Proceed
}

fn update_selection_after_filter(listbox: &ListBox, preview_area_rc_opt: &Option<std::rc::Rc<std::cell::RefCell<preview::PreviewArea>>>) {
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

// --- Item Loading ---

fn load_items_from_config(
    listbox: &ListBox,
    config_path: &str,
    category_filter: &Option<String>,
    preview_area_rc_opt: Option<std::rc::Rc<std::cell::RefCell<preview::PreviewArea>>>,
) {
    let config_path = config_path.to_string();
    let category_filter = category_filter.clone();
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

        // If a category is specified, load only items from that category, otherwise load only categories with the same mode as the global default
        if let Some(ref category) = category_filter {
            if let Some(category_config) = config.categories.get(category) {
                let effective_mode = category_config.mode.clone().unwrap_or(config.mode.clone());
                for (key, value) in &category_config.entries {
                    items.push(Item {
                        title: key.clone(),
                        value: value.clone(),
                        category: category.clone(),
                        mode: effective_mode.clone(),
                    });
                }
            }
        } else {
            // Load items only from categories that match the global default mode
            for (category_name, category_config) in &config.categories {
                let effective_mode = category_config.mode.clone().unwrap_or(config.mode.clone());
                if effective_mode == config.mode {
                    for (key, value) in &category_config.entries {
                        items.push(Item {
                            title: key.clone(),
                            value: value.clone(),
                            category: category_name.clone(),
                            mode: effective_mode.clone(),
                        });
                    }
                }
            }
        }

        use rayon::prelude::*;
        let all_paths: Vec<_> = items
            .par_iter()
            .flat_map(|item| {
                if let Mode::Picture = item.mode {
                    let expanded_path = utils::expand_tilde(&item.value);
                    let expanded_path_str = expanded_path.to_string_lossy().to_string();

                    if utils::is_path_directory(&expanded_path_str) {
                        use walkdir::WalkDir;
                        let mut paths = Vec::new();
                        for entry in WalkDir::new(&expanded_path_str).follow_links(true) {
                            if let Ok(entry) = entry {
                                let path = entry.path();
                                if path.is_file() {
                                    let path_str = path.to_string_lossy();
                                    if utils::is_image_file(&path_str) {
                                        paths.push(Item {
                                            title: format!("{} ({})", path.file_name().unwrap_or_default().to_string_lossy(), item.title),
                                            value: path_str.to_string(),
                                            category: item.category.clone(),
                                            mode: item.mode.clone(),
                                        });
                                    }
                                }
                            }
                        }
                        paths
                    } else {
                        vec![Item {
                            title: item.title.clone(),
                            value: expanded_path_str,
                            category: item.category.clone(),
                            mode: item.mode.clone(),
                        }]
                    }
                } else {
                    vec![item.clone()]
                }
            })
            .collect();

        let items = all_paths;

        let items_clone = items.clone();
        let listbox_weak_clone = listbox_weak.clone();
        glib::idle_add_local(move || {
            if let Some(listbox_strong) = listbox_weak_clone.upgrade() {
                for item in &items_clone {
                    add_item_to_ui(&listbox_strong, item.clone());
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

fn add_item_to_ui(listbox: &ListBox, item: Item) {
    let row = list::create_list_item(&item.title, &item.value);
    unsafe { row.set_data("item", item); }
    listbox.append(&row);
}
