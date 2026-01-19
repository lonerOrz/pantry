use gtk4::prelude::ObjectExt;
use gtk4::{
    prelude::*, Application, ApplicationWindow, Box as GtkBox, Label, ListBox, Orientation,
    Overlay, ScrolledWindow,
};
use std::cell::RefCell;
use std::io::Read;
use std::rc::Rc;

use crate::app::app::Args;
use crate::config::{get_config_display_mode, resolve_display_mode, DisplayMode, SourceMode};
use crate::domain::item::Item;
use crate::ui::{list, preview, window};
use crate::window_state::WindowState;

pub struct UiBuilder;

impl UiBuilder {
    pub fn build_stdin_ui(
        args: &Args,
        window_state: &WindowState,
        app: &Application,
    ) -> (
        ApplicationWindow,
        ListBox,
        Option<Rc<RefCell<preview::PreviewArea>>>,
        Label,
    ) {
        // 从 stdin 读取数据
        let mut stdin_data = String::new();
        if let Err(e) = std::io::stdin().read_to_string(&mut stdin_data) {
            eprintln!("Warning: Failed to read from stdin: {}", e);
            // Continue with empty data
        }

        let window = window::create_main_window(app, args);
        window.set_default_size(window_state.width, window_state.height);

        if window_state.maximized {
            window.maximize();
        }

        // 根据命令行参数或默认值确定显示模式
        let display_mode = resolve_display_mode(&args.display, &None, &DisplayMode::Text);

        let (main_widget, listbox, preview_area_rc_opt) =
            if matches!(display_mode, DisplayMode::Picture) {
                let paned = gtk4::Paned::new(gtk4::Orientation::Horizontal);

                let listbox = list::create_listbox();
                let scrolled = wrap_in_scroll(&listbox);
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

                paned.set_position(crate::constants::DEFAULT_PANED_POSITION);

                let preview_area_rc = std::rc::Rc::new(std::cell::RefCell::new(preview_area));

                (
                    paned.upcast::<gtk4::Widget>(),
                    listbox,
                    Some(preview_area_rc),
                )
            } else {
                let layout = GtkBox::new(Orientation::Vertical, 0);
                let listbox = list::create_listbox();
                let scrolled = wrap_in_scroll(&listbox);
                layout.append(&scrolled);

                (layout.upcast::<gtk4::Widget>(), listbox, None)
            };

        let (overlay, search_label) = create_search_overlay(&main_widget);
        window.set_child(Some(&overlay));

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

                add_item_to_ui(&listbox, item);
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
                glib::timeout_add_local(
                    std::time::Duration::from_millis(crate::constants::PREVIEW_UPDATE_THROTTLE_MS),
                    move || {
                        update_preview(&listbox_inner, &preview_area_rc_opt_inner);
                        glib::ControlFlow::Break
                    },
                );
            });

            // Trigger initial preview update
            glib::timeout_add_local(
                std::time::Duration::from_millis(crate::constants::INITIAL_PREVIEW_DELAY_MS),
                {
                    let listbox_clone = listbox.clone();
                    let preview_area_rc_opt_clone = preview_area_rc_opt.clone();
                    move || {
                        update_preview(&listbox_clone, &preview_area_rc_opt_clone);
                        glib::ControlFlow::Break
                    }
                },
            );

            if let Some(paned_widget) = main_widget.downcast_ref::<gtk4::Paned>() {
                let window_clone = window.clone();
                let paned_widget_clone = paned_widget.clone();
                glib::timeout_add_local(
                    std::time::Duration::from_millis(
                        crate::constants::SELECTION_UPDATE_DELAY_MS * 10,
                    ),
                    move || {
                        let allocation = window_clone.default_size();
                        let width = allocation.0;

                        let position =
                            (width as f64 * crate::constants::MAX_WINDOW_WIDTH_FRACTION) as i32;
                        paned_widget_clone.set_position(position);

                        glib::ControlFlow::Break
                    },
                );

                let paned_widget_clone2 = paned_widget.clone();
                window.connect_realize(move |win| {
                    let win_clone = win.clone();
                    let paned_widget_clone3 = paned_widget_clone2.clone();
                    glib::timeout_add_local(
                        std::time::Duration::from_millis(
                            crate::constants::SELECTION_UPDATE_DELAY_MS,
                        ),
                        move || {
                            let allocation = win_clone.default_size();
                            let width = allocation.0;

                            let position =
                                (width as f64 * crate::constants::MAX_WINDOW_WIDTH_FRACTION) as i32;
                            paned_widget_clone3.set_position(position);

                            glib::ControlFlow::Break
                        },
                    );
                });
            }
        }

        (window, listbox, preview_area_rc_opt, search_label)
    }

    pub fn build_config_ui(
        args: &Args,
        window_state: &WindowState,
        app: &Application,
    ) -> (
        ApplicationWindow,
        ListBox,
        Option<Rc<RefCell<preview::PreviewArea>>>,
        Label,
    ) {
        let window = window::create_main_window(app, args);
        window.set_default_size(window_state.width, window_state.height);

        if window_state.maximized {
            window.maximize();
        }

        let config_path = args.config.as_ref().unwrap();

        let (main_widget, listbox, preview_area_rc_opt) = if matches!(
            get_config_display_mode(config_path, &args.category, &args.display),
            DisplayMode::Picture
        ) {
            let paned = gtk4::Paned::new(gtk4::Orientation::Horizontal);

            let listbox = list::create_listbox();
            let scrolled = wrap_in_scroll(&listbox);
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

            paned.set_position(crate::constants::DEFAULT_PANED_POSITION);

            let preview_area_rc = std::rc::Rc::new(std::cell::RefCell::new(preview_area));

            (
                paned.upcast::<gtk4::Widget>(),
                listbox,
                Some(preview_area_rc),
            )
        } else {
            let layout = GtkBox::new(Orientation::Vertical, 0);
            let listbox = list::create_listbox();
            let scrolled = wrap_in_scroll(&listbox);
            layout.append(&scrolled);

            (layout.upcast::<gtk4::Widget>(), listbox, None)
        };

        let (overlay, search_label) = create_search_overlay(&main_widget);
        window.set_child(Some(&overlay));

        (window, listbox, preview_area_rc_opt, search_label)
    }
}

fn create_search_overlay(child: &impl gtk4::prelude::IsA<gtk4::Widget>) -> (Overlay, gtk4::Label) {
    let overlay = Overlay::new();
    overlay.set_child(Some(child));
    let label = gtk4::Label::new(None);
    label.add_css_class("app-notification");
    label.add_css_class("hidden");
    label.set_halign(gtk4::Align::Center);
    label.set_valign(gtk4::Align::End);
    label.set_margin_bottom(crate::constants::WINDOW_MARGIN_BOTTOM);
    overlay.add_overlay(&label);
    (overlay, label)
}

fn wrap_in_scroll(child: &impl gtk4::prelude::IsA<gtk4::Widget>) -> ScrolledWindow {
    let scrolled = ScrolledWindow::new();
    scrolled.set_child(Some(child));
    scrolled.set_vexpand(true);
    scrolled
}

fn add_item_to_ui(listbox: &ListBox, item: Item) {
    let row = list::create_list_item(&item.title, &item.value);
    let item_obj = crate::app::item_object::ItemObject::new(item);
    unsafe {
        row.set_data("item", item_obj);
    }
    listbox.append(&row);
}

fn update_preview(
    listbox: &ListBox,
    preview_area_rc_opt: &Option<Rc<RefCell<preview::PreviewArea>>>,
) {
    use std::sync::atomic::{AtomicU64, Ordering};
    use std::sync::OnceLock;
    use std::time::{SystemTime, UNIX_EPOCH};

    static LAST_UPDATE_TIME: OnceLock<AtomicU64> = OnceLock::new();
    let last_update = LAST_UPDATE_TIME.get_or_init(|| AtomicU64::new(0));

    let now = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|d| d.as_millis() as u64)
        .unwrap_or(0);

    let prev_time = last_update.load(Ordering::Relaxed);
    // Skip throttling for initial update (when prev_time is 0) or if enough time has passed
    if prev_time != 0
        && now.saturating_sub(prev_time) < crate::constants::PREVIEW_UPDATE_THROTTLE_MS
    {
        return;
    }

    // Attempt to update the timestamp atomically
    if !last_update
        .compare_exchange(prev_time, now, Ordering::Relaxed, Ordering::Relaxed)
        .is_ok()
    {
        // Another thread updated the time, skip this update
        return;
    }

    if let Some(preview_area_rc) = preview_area_rc_opt {
        if let Some(selected_row) = listbox.selected_row() {
            if let Some(item_obj_ptr) =
                unsafe { selected_row.data::<crate::app::item_object::ItemObject>("item") }
            {
                let item_obj = unsafe { &*item_obj_ptr.as_ptr() };
                if let Some(item) = item_obj.item() {
                    let preview_area = &*preview_area_rc.borrow();
                    preview_area.update_with_content(&item);
                }
            }
        }
    }
}
