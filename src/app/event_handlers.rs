use crate::ui::list::ListState;
use gtk4::gdk::ModifierType;
use gtk4::{ApplicationWindow, EventControllerKey, ListScrollFlags, PropagationPhase, prelude::*};

pub fn setup_keyboard_controller(
    window: &ApplicationWindow,
    list_state: &ListState,
    search_entry: &gtk4::SearchEntry,
) {
    let controller = EventControllerKey::new();
    controller.set_propagation_phase(PropagationPhase::Capture);

    let list_state = list_state.clone();
    let search_entry = search_entry.clone();

    controller.connect_key_pressed(move |controller, keyval, _, state| {
        let has_ctrl = state.contains(ModifierType::CONTROL_MASK);

        if keyval == gtk4::gdk::Key::Return || keyval == gtk4::gdk::Key::KP_Enter {
            handle_selection(&list_state);
            return glib::Propagation::Stop;
        }

        let is_cancel = keyval == gtk4::gdk::Key::Escape
            || (has_ctrl && (keyval == gtk4::gdk::Key::g || keyval == gtk4::gdk::Key::c));

        if is_cancel {
            if !search_entry.text().is_empty() && keyval == gtk4::gdk::Key::Escape {
                search_entry.set_text("");
            } else if let Some(win) = list_state.view.root().and_downcast::<ApplicationWindow>() {
                win.close();
            }
            return glib::Propagation::Stop;
        }

        let is_up = keyval == gtk4::gdk::Key::Up
            || (has_ctrl && (keyval == gtk4::gdk::Key::p || keyval == gtk4::gdk::Key::k));

        let is_down = keyval == gtk4::gdk::Key::Down
            || (has_ctrl && (keyval == gtk4::gdk::Key::n || keyval == gtk4::gdk::Key::j));

        if is_up {
            let current = list_state.selection.selected();
            if current != gtk4::INVALID_LIST_POSITION && current > 0 {
                list_state.selection.set_selected(current - 1);
                list_state
                    .view
                    .scroll_to(current - 1, ListScrollFlags::FOCUS, None);
            } else {
                controller.forward(&list_state.view);
            }
            return glib::Propagation::Stop;
        }

        if is_down {
            let current = list_state.selection.selected();
            let total = list_state.sort_model.n_items();
            if current == gtk4::INVALID_LIST_POSITION {
                if total > 0 {
                    list_state.selection.set_selected(0);
                    list_state.view.scroll_to(0, ListScrollFlags::FOCUS, None);
                }
            } else if current + 1 < total {
                list_state.selection.set_selected(current + 1);
                list_state
                    .view
                    .scroll_to(current + 1, ListScrollFlags::FOCUS, None);
            } else {
                controller.forward(&list_state.view);
            }
            return glib::Propagation::Stop;
        }

        if has_ctrl && keyval == gtk4::gdk::Key::u {
            search_entry.set_text("");
            return glib::Propagation::Stop;
        }

        glib::Propagation::Proceed
    });
    window.add_controller(controller);
}

pub fn handle_selection(list_state: &ListState) {
    let Some(item) = list_state.selected_item() else {
        return;
    };

    print!("{}", item.value);
    let _ = std::io::Write::flush(&mut std::io::stdout());

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
