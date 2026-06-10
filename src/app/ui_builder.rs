use gtk4::{
    Align, Application, ApplicationWindow, Box as GtkBox, Label, Orientation, Overlay,
    ScrolledWindow, prelude::*,
};
use std::cell::RefCell;
use std::io::Read;
use std::rc::Rc;

use crate::app::application::Args;
use crate::config::{DisplayMode, SourceMode, get_config_display_mode, resolve_display_mode};
use crate::domain::item::Item;
use crate::ui::{list::ListState, preview, window};
use crate::window_state::WindowState;

pub struct UiBuilder;

impl UiBuilder {
    pub fn build_stdin_ui(
        args: &Args,
        window_state: &WindowState,
        app: &Application,
        query_state: crate::ui::search::SearchState,
    ) -> (
        ApplicationWindow,
        ListState,
        Option<Rc<RefCell<preview::PreviewArea>>>,
        Label,
    ) {
        let mut stdin_data = String::new();
        if let Err(e) = std::io::stdin().read_to_string(&mut stdin_data) {
            eprintln!("Warning: Failed to read from stdin: {}", e);
        }

        let window = window::create_main_window(app, args);
        window.set_default_size(window_state.width, window_state.height);
        window.add_css_class("transparent-window-bg");

        if window_state.maximized {
            window.maximize();
        }

        let display_mode = resolve_display_mode(&args.display, &None, &DisplayMode::Text);
        let list_state = ListState::new(query_state);
        let (main_widget, preview_area_rc_opt) =
            build_main_widget(&list_state, display_mode.clone());

        let frame_wrapper = GtkBox::new(Orientation::Vertical, 0);
        frame_wrapper.add_css_class("pantry-main-frame");
        frame_wrapper.append(&main_widget);

        let (overlay, search_label) = create_search_overlay(&frame_wrapper);
        window.set_child(Some(&overlay));

        for line in stdin_data.lines().filter(|line| !line.trim().is_empty()) {
            list_state.append_item(Item {
                title: line.to_string(),
                value: line.to_string(),
                category: "stdin".to_string(),
                display: display_mode.clone(),
                source: SourceMode::Config,
                preview_template: None,
            });
        }

        list_state.select_first();
        setup_preview_updates(
            &window,
            &main_widget,
            &list_state,
            &preview_area_rc_opt,
            display_mode,
        );

        (window, list_state, preview_area_rc_opt, search_label)
    }

    pub fn build_config_ui(
        args: &Args,
        window_state: &WindowState,
        app: &Application,
        query_state: crate::ui::search::SearchState,
    ) -> (
        ApplicationWindow,
        ListState,
        Option<Rc<RefCell<preview::PreviewArea>>>,
        Label,
    ) {
        let window = window::create_main_window(app, args);
        window.set_default_size(window_state.width, window_state.height);
        window.add_css_class("transparent-window-bg");

        if window_state.maximized {
            window.maximize();
        }

        let display_mode = get_config_display_mode(&args.config, &args.category, &args.display);
        let list_state = ListState::new(query_state);
        let (main_widget, preview_area_rc_opt) =
            build_main_widget(&list_state, display_mode.clone());

        let frame_wrapper = GtkBox::new(Orientation::Vertical, 0);
        frame_wrapper.add_css_class("pantry-main-frame");
        frame_wrapper.append(&main_widget);

        let (overlay, search_label) = create_search_overlay(&frame_wrapper);
        window.set_child(Some(&overlay));
        setup_preview_updates(
            &window,
            &main_widget,
            &list_state,
            &preview_area_rc_opt,
            display_mode,
        );

        (window, list_state, preview_area_rc_opt, search_label)
    }
}

fn build_main_widget(
    list_state: &ListState,
    display_mode: DisplayMode,
) -> (gtk4::Widget, Option<Rc<RefCell<preview::PreviewArea>>>) {
    let (content_widget, preview_area_rc_opt) = build_content(list_state, display_mode);

    let list_stack = gtk4::Stack::new();
    list_stack.set_transition_type(gtk4::StackTransitionType::Crossfade);
    list_stack.set_transition_duration(150);

    list_stack.add_titled(&content_widget, Some("content"), "Content");

    let empty_box = gtk4::Box::new(Orientation::Vertical, 0);
    empty_box.set_valign(Align::Center);
    empty_box.set_halign(Align::Center);
    empty_box.add_css_class("empty-placeholder-box");

    let icon_wrapper = gtk4::Box::new(Orientation::Vertical, 0);
    icon_wrapper.add_css_class("empty-placeholder-icon-wrapper");
    icon_wrapper.set_halign(Align::Center);
    icon_wrapper.set_valign(Align::Center);

    let empty_icon = gtk4::Image::from_icon_name("system-search-symbolic");
    empty_icon.set_pixel_size(36);
    icon_wrapper.append(&empty_icon);

    let empty_label = Label::new(Some("No Matching Results"));
    empty_label.add_css_class("empty-placeholder-text");

    let empty_sub_label = Label::new(Some("Try entering different terms or check your spelling."));
    empty_sub_label.add_css_class("empty-placeholder-subtitle");

    empty_box.append(&icon_wrapper);
    empty_box.append(&empty_label);
    empty_box.append(&empty_sub_label);

    list_stack.add_titled(&empty_box, Some("empty"), "Empty");

    let list_stack_clone = list_stack.clone();
    let sort_model_clone = list_state.sort_model.clone();

    if sort_model_clone.n_items() == 0 {
        list_stack_clone.set_visible_child_name("empty");
    } else {
        list_stack_clone.set_visible_child_name("content");
    }

    sort_model_clone.connect_items_changed(move |model, _, _, _| {
        if model.n_items() == 0 {
            list_stack_clone.set_visible_child_name("empty");
        } else {
            list_stack_clone.set_visible_child_name("content");
        }
    });

    (list_stack.upcast::<gtk4::Widget>(), preview_area_rc_opt)
}

fn build_content(
    list_state: &ListState,
    display_mode: DisplayMode,
) -> (gtk4::Widget, Option<Rc<RefCell<preview::PreviewArea>>>) {
    if matches!(display_mode, DisplayMode::Picture) {
        let paned = gtk4::Paned::new(gtk4::Orientation::Horizontal);

        let scrolled = wrap_in_scroll(&list_state.view);
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

        (
            paned.upcast::<gtk4::Widget>(),
            Some(Rc::new(RefCell::new(preview_area))),
        )
    } else {
        let layout = GtkBox::new(Orientation::Vertical, 0);
        let scrolled = wrap_in_scroll(&list_state.view);
        layout.append(&scrolled);

        (layout.upcast::<gtk4::Widget>(), None)
    }
}

fn setup_preview_updates(
    window: &ApplicationWindow,
    main_widget: &gtk4::Widget,
    list_state: &ListState,
    preview_area_rc_opt: &Option<Rc<RefCell<preview::PreviewArea>>>,
    display_mode: DisplayMode,
) {
    if !matches!(display_mode, DisplayMode::Picture) {
        return;
    }

    let preview_area_rc_opt_clone = preview_area_rc_opt.clone();
    let list_state_clone = list_state.clone();

    let active_timeout_id = Rc::new(RefCell::new(None::<glib::SourceId>));

    list_state
        .selection
        .connect_selection_changed(move |_, _, _| {
            let preview_area_rc_opt_inner = preview_area_rc_opt_clone.clone();
            let list_state_inner = list_state_clone.clone();
            let active_timeout_id_inner = active_timeout_id.clone();

            if let Some(old_id) = active_timeout_id_inner.borrow_mut().take() {
                old_id.remove();
            }

            let new_id = glib::timeout_add_local(
                std::time::Duration::from_millis(crate::constants::PREVIEW_UPDATE_THROTTLE_MS),
                move || {
                    active_timeout_id_inner.borrow_mut().take();

                    crate::app::preview_manager::PreviewManager::update_preview(
                        &list_state_inner,
                        &preview_area_rc_opt_inner,
                    );
                    glib::ControlFlow::Break
                },
            );

            active_timeout_id.borrow_mut().replace(new_id);
        });

    glib::timeout_add_local(
        std::time::Duration::from_millis(crate::constants::INITIAL_PREVIEW_DELAY_MS),
        {
            let list_state_clone = list_state.clone();
            let preview_area_rc_opt_clone = preview_area_rc_opt.clone();
            move || {
                crate::app::preview_manager::PreviewManager::update_preview(
                    &list_state_clone,
                    &preview_area_rc_opt_clone,
                );
                glib::ControlFlow::Break
            }
        },
    );

    if let Some(paned_widget) = main_widget.downcast_ref::<gtk4::Paned>() {
        let window_clone = window.clone();
        let paned_widget_clone = paned_widget.clone();
        glib::timeout_add_local(
            std::time::Duration::from_millis(crate::constants::SELECTION_UPDATE_DELAY_MS * 10),
            move || {
                let width = window_clone.default_size().0;
                let position = (width as f64 * crate::constants::MAX_WINDOW_WIDTH_FRACTION) as i32;
                paned_widget_clone.set_position(position);
                glib::ControlFlow::Break
            },
        );

        let paned_widget_clone = paned_widget.clone();
        window.connect_realize(move |win| {
            let win_clone = win.clone();
            let paned_widget_clone = paned_widget_clone.clone();
            glib::timeout_add_local(
                std::time::Duration::from_millis(crate::constants::SELECTION_UPDATE_DELAY_MS),
                move || {
                    let width = win_clone.default_size().0;
                    let position =
                        (width as f64 * crate::constants::MAX_WINDOW_WIDTH_FRACTION) as i32;
                    paned_widget_clone.set_position(position);
                    glib::ControlFlow::Break
                },
            );
        });
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
