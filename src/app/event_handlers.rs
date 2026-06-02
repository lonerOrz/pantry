use crate::app::application::Args;
use crate::app::preview_manager::PreviewManager;
use crate::ui::list::ListState;
use gtk4::{ApplicationWindow, EventControllerKey, Label, PropagationPhase, prelude::*};

pub struct EventHandler;

impl EventHandler {
    pub fn setup_keyboard_controller(
        window: &ApplicationWindow,
        list_state: &ListState,
        query_state: crate::ui::search::SearchState,
        search_label: Label,
        _args: &Args,
        preview_area_rc_opt: Option<
            std::rc::Rc<std::cell::RefCell<crate::ui::preview::PreviewArea>>,
        >,
    ) {
        let controller = EventControllerKey::new();
        controller.set_propagation_phase(PropagationPhase::Capture);

        let list_state = list_state.clone();
        let search_label = search_label.clone();
        let preview_area_rc = preview_area_rc_opt;

        controller.connect_key_pressed(move |_, keyval, _, _| {
            if keyval == gtk4::gdk::Key::Return || keyval == gtk4::gdk::Key::KP_Enter {
                handle_selection(&list_state);
                return glib::Propagation::Stop;
            }

            if keyval == gtk4::gdk::Key::Escape {
                clear_search(&query_state, &list_state, &search_label, &preview_area_rc);
                return glib::Propagation::Stop;
            }

            handle_search_input(
                keyval,
                &query_state,
                &list_state,
                &search_label,
                &preview_area_rc,
            )
        });
        window.add_controller(controller);
    }
}

pub fn handle_selection(list_state: &ListState) {
    let Some(item) = list_state.selected_item() else {
        return;
    };

    print!("{}", item.value);

    use std::io::{self, Write};
    let _ = io::stdout().flush();

    if let Some(window) = list_state.view.root().and_downcast::<ApplicationWindow>() {
        let (width, height) = window.default_size();
        let state = crate::window_state::WindowState {
            width,
            height,
            maximized: window.is_maximized(),
        };
        state.save();
        window.close();
    }
}

pub fn clear_search(
    query_state: &crate::ui::search::SearchState,
    list_state: &ListState,
    label: &Label,
    preview_area_rc_opt: &Option<std::rc::Rc<std::cell::RefCell<crate::ui::preview::PreviewArea>>>,
) {
    query_state.borrow_mut().clear();
    label.add_css_class("hidden");
    update_selection_after_filter(list_state, preview_area_rc_opt);
}

pub fn update_selection_after_filter(
    list_state: &ListState,
    preview_area_rc_opt: &Option<std::rc::Rc<std::cell::RefCell<crate::ui::preview::PreviewArea>>>,
) {
    crate::app::search_logic::SearchLogic::refresh_filter(list_state);
    list_state.select_first();
    PreviewManager::update_preview(list_state, preview_area_rc_opt);
}

pub fn handle_search_input(
    keyval: gtk4::gdk::Key,
    query_state: &crate::ui::search::SearchState,
    list_state: &ListState,
    label: &Label,
    preview_area_rc_opt: &Option<std::rc::Rc<std::cell::RefCell<crate::ui::preview::PreviewArea>>>,
) -> glib::Propagation {
    let (should_invalidate, current_text) = {
        let mut query = query_state.borrow_mut();
        let mut updated = false;
        if keyval == gtk4::gdk::Key::BackSpace {
            query.pop();
            updated = true;
        } else if let Some(c) = keyval.to_unicode()
            && (c.is_alphanumeric() || matches!(c, '-' | '_' | '.' | '/' | '@'))
        {
            query.push(c);
            updated = true;
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
        update_selection_after_filter(list_state, preview_area_rc_opt);
        return glib::Propagation::Stop;
    }

    glib::Propagation::Proceed
}
