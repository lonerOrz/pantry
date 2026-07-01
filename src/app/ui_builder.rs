use gtk4::{
    Application, ApplicationWindow, Box as GtkBox, Orientation, ScrolledWindow, SearchEntry,
    prelude::*,
};
use std::cell::RefCell;
use std::rc::Rc;

use crate::constants::MAX_ITEMS;
use crate::domain::DisplayMode;
use crate::domain::item::Item;
use crate::ui::{header, list::ListState, preview, window};
use crate::window_state::WindowState;

use crate::app::preview_manager::PreviewUpdater;

pub enum UiMode {
    Stdin,
    Config { display_mode: DisplayMode },
}

pub fn build_ui(
    window_state: &WindowState,
    app: &Application,
    query_state: crate::ui::search::SearchState,
    mode: UiMode,
    preview_manager: &Rc<RefCell<dyn PreviewUpdater>>,
) -> (
    ApplicationWindow,
    ListState,
    Option<Rc<RefCell<preview::PreviewArea>>>,
    SearchEntry,
) {
    let display_mode = match &mode {
        UiMode::Stdin => DisplayMode::Text,
        UiMode::Config { display_mode } => display_mode.clone(),
    };

    let (window, list_state, preview_area_rc_opt, search_entry, main_widget) = build_ui_shell(
        window_state,
        app,
        &query_state,
        &display_mode,
        preview_manager,
    );

    if matches!(mode, UiMode::Config { .. }) && window_state.maximized {
        window.maximize();
    }

    match mode {
        UiMode::Stdin => spawn_stdin_reader(&list_state, &display_mode),
        UiMode::Config { .. } => {}
    }

    list_state.view.grab_focus();

    setup_preview_updates(
        &window,
        &main_widget,
        &list_state,
        &preview_area_rc_opt,
        display_mode,
        preview_manager,
    );

    (window, list_state, preview_area_rc_opt, search_entry)
}

fn build_ui_shell(
    window_state: &WindowState,
    app: &Application,
    query_state: &crate::ui::search::SearchState,
    display_mode: &DisplayMode,
    preview_manager: &Rc<RefCell<dyn PreviewUpdater>>,
) -> (
    ApplicationWindow,
    ListState,
    Option<Rc<RefCell<preview::PreviewArea>>>,
    SearchEntry,
    gtk4::Widget,
) {
    let window = window::create_main_window(app);
    window.set_default_size(window_state.width, window_state.height);

    let list_state = ListState::new(query_state.clone());
    let (main_widget, preview_area_rc_opt) = build_main_widget(&list_state, display_mode.clone());

    let (header_bar, search_entry, menu_button) = header::build_header_bar();
    header::connect_about_dialog(&window, &menu_button);

    let frame_wrapper = GtkBox::new(Orientation::Vertical, 0);
    frame_wrapper.add_css_class("pantry-main-frame");
    frame_wrapper.append(&header_bar);
    frame_wrapper.append(&main_widget);

    window.set_child(Some(&frame_wrapper));

    search_entry.set_key_capture_widget(Some(&window));

    let preview_manager_clone = preview_manager.clone();
    let list_state_clone = list_state.clone();
    let preview_area_rc_opt_clone = preview_area_rc_opt.clone();

    header::connect_search_changed(&search_entry, &list_state, query_state, move || {
        preview_manager_clone
            .borrow()
            .update_preview(&list_state_clone, &preview_area_rc_opt_clone);
    });

    (
        window,
        list_state,
        preview_area_rc_opt,
        search_entry,
        main_widget,
    )
}

fn spawn_stdin_reader(list_state: &ListState, display_mode: &DisplayMode) {
    let (tx, rx) = std::sync::mpsc::channel::<String>();

    std::thread::spawn(move || {
        use std::io::BufRead;
        let stdin = std::io::stdin();
        let reader = stdin.lock();
        for line in reader.lines().map_while(Result::ok) {
            if !line.trim().is_empty() {
                let _ = tx.send(line);
            }
        }
    });

    let list_state_clone = list_state.clone();
    let display_mode_clone = display_mode.clone();
    let mut stdin_count: usize = 0;

    glib::idle_add_local(move || {
        use std::sync::mpsc::TryRecvError;

        let mut batch_count = 0;

        while batch_count < 100 {
            match rx.try_recv() {
                Ok(line) => {
                    list_state_clone.append_item(Item::stdin(line, display_mode_clone.clone()));
                    stdin_count += 1;
                    batch_count += 1;

                    if stdin_count >= MAX_ITEMS {
                        return glib::ControlFlow::Break;
                    }
                }
                Err(TryRecvError::Empty) => break,
                Err(TryRecvError::Disconnected) => return glib::ControlFlow::Break,
            }
        }

        if list_state_clone.selection.selected() == gtk4::INVALID_LIST_POSITION && stdin_count > 0 {
            list_state_clone.select_first();
        }

        glib::ControlFlow::Continue
    });
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

    let empty_box = GtkBox::new(Orientation::Vertical, 0);
    empty_box.set_valign(gtk4::Align::Center);
    empty_box.set_halign(gtk4::Align::Center);
    empty_box.add_css_class("empty-placeholder-box");

    let icon_wrapper = GtkBox::new(Orientation::Vertical, 0);
    icon_wrapper.add_css_class("empty-placeholder-icon-wrapper");
    icon_wrapper.set_halign(gtk4::Align::Center);
    icon_wrapper.set_valign(gtk4::Align::Center);

    let empty_icon = gtk4::Image::from_icon_name("system-search-symbolic");
    empty_icon.set_pixel_size(36);
    icon_wrapper.append(&empty_icon);

    let empty_label = gtk4::Label::new(Some("No Matching Results"));
    empty_label.add_css_class("empty-placeholder-text");

    let empty_sub_label =
        gtk4::Label::new(Some("Try entering different terms or check your spelling."));
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
    preview_manager: &Rc<RefCell<dyn PreviewUpdater>>,
) {
    if !matches!(display_mode, DisplayMode::Picture) {
        return;
    }

    let preview_area_rc_opt_clone1 = preview_area_rc_opt.clone();
    let preview_area_rc_opt_clone2 = preview_area_rc_opt.clone();
    let list_state_clone = list_state.clone();
    let preview_manager_clone1 = preview_manager.clone();
    let preview_manager_clone2 = preview_manager.clone();

    let active_timeout_id = Rc::new(RefCell::new(None::<glib::SourceId>));

    list_state
        .selection
        .connect_selection_changed(move |_, _, _| {
            let preview_area_rc_opt_inner = preview_area_rc_opt_clone1.clone();
            let list_state_inner = list_state_clone.clone();
            let active_timeout_id_inner = active_timeout_id.clone();
            let preview_manager_inner = preview_manager_clone1.clone();

            if let Some(old_id) = active_timeout_id_inner.borrow_mut().take() {
                old_id.remove();
            }

            let new_id = glib::timeout_add_local(
                std::time::Duration::from_millis(crate::constants::PREVIEW_UPDATE_THROTTLE_MS),
                move || {
                    active_timeout_id_inner.borrow_mut().take();

                    preview_manager_inner
                        .borrow()
                        .update_preview(&list_state_inner, &preview_area_rc_opt_inner);
                    glib::ControlFlow::Break
                },
            );

            active_timeout_id.borrow_mut().replace(new_id);
        });

    glib::timeout_add_local(
        std::time::Duration::from_millis(crate::constants::INITIAL_PREVIEW_DELAY_MS),
        {
            let list_state_clone = list_state.clone();
            move || {
                preview_manager_clone2
                    .borrow()
                    .update_preview(&list_state_clone, &preview_area_rc_opt_clone2);
                glib::ControlFlow::Break
            }
        },
    );

    if let Some(paned_widget) = main_widget.downcast_ref::<gtk4::Paned>() {
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

fn wrap_in_scroll(child: &impl gtk4::prelude::IsA<gtk4::Widget>) -> ScrolledWindow {
    let scrolled = ScrolledWindow::new();
    scrolled.set_child(Some(child));
    scrolled.set_vexpand(true);
    scrolled
}
