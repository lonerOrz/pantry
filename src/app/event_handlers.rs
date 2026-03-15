use crate::app::application::Args;
use crate::app::preview_manager::PreviewManager;
use gtk4::{
    ApplicationWindow, EventControllerKey, Label, ListBox, ListBoxRow, PropagationPhase, prelude::*,
};

pub struct EventHandler;

impl EventHandler {
    pub fn setup_keyboard_controller(
        window: &ApplicationWindow,
        listbox: &ListBox,
        query_state: crate::ui::search::SearchState,
        search_label: Label,
        _args: &Args,
        preview_area_rc_opt: Option<
            std::rc::Rc<std::cell::RefCell<crate::ui::preview::PreviewArea>>,
        >,
    ) {
        let controller = EventControllerKey::new();
        controller.set_propagation_phase(PropagationPhase::Capture);
        let listbox = listbox.clone();
        let search_label = search_label.clone();
        // Determine preview enabled based on the preview_area_rc_opt parameter
        let preview_enabled = preview_area_rc_opt.is_some();
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
                    PreviewManager::update_preview(&listbox_clone, &preview_area_rc_clone);
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
}

// Standalone functions for UI event handling to avoid lifetime issues
pub fn handle_selection(listbox: &ListBox) {
    if let Some(selected_row) = listbox.selected_row() {
        if let Some(item_obj) = crate::app::item_object::ItemObject::from_row(&selected_row) {
            if let Some(item) = item_obj.item() {
                print!("{}", item.value);

                use std::io::{self, Write};
                let _ = io::stdout().flush();

                if let Some(window) = listbox.root().and_downcast::<ApplicationWindow>() {
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
        }
    }
}

pub fn clear_search(
    query_state: &crate::ui::search::SearchState,
    listbox: &ListBox,
    label: &Label,
    preview_area_rc_opt: &Option<std::rc::Rc<std::cell::RefCell<crate::ui::preview::PreviewArea>>>,
) {
    query_state.borrow_mut().clear();
    label.add_css_class("hidden");
    listbox.invalidate_filter();
    update_selection_after_filter(listbox, preview_area_rc_opt);
}

pub fn update_selection_after_filter(
    listbox: &ListBox,
    preview_area_rc_opt: &Option<std::rc::Rc<std::cell::RefCell<crate::ui::preview::PreviewArea>>>,
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
    PreviewManager::update_preview(listbox, preview_area_rc_opt);
}

pub fn first_visible_row_after_filter(listbox: &ListBox) -> Option<ListBoxRow> {
    let mut i = 0;
    while let Some(row) = listbox.row_at_index(i) {
        if row.is_child_visible() {
            return Some(row);
        }
        i += 1;
    }
    None
}

pub fn handle_search_input(
    keyval: gtk4::gdk::Key,
    query_state: &crate::ui::search::SearchState,
    listbox: &ListBox,
    label: &Label,
    preview_area_rc_opt: &Option<std::rc::Rc<std::cell::RefCell<crate::ui::preview::PreviewArea>>>,
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
