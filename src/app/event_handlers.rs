use crate::ui::list::ListState;
use gtk4::{ApplicationWindow, EventControllerKey, PropagationPhase, prelude::*};

pub fn setup_keyboard_controller(
    window: &ApplicationWindow,
    list_state: &ListState,
    search_entry: &gtk4::SearchEntry,
    _preview_area_rc_opt: Option<
        std::rc::Rc<std::cell::RefCell<crate::ui::preview::PreviewArea>>,
    >,
) {
    let controller = EventControllerKey::new();
    controller.set_propagation_phase(PropagationPhase::Capture);

    let list_state = list_state.clone();
    let search_entry = search_entry.clone();

    controller.connect_key_pressed(move |controller, keyval, _, _| {
        if keyval == gtk4::gdk::Key::Return || keyval == gtk4::gdk::Key::KP_Enter {
            handle_selection(&list_state);
            return glib::Propagation::Stop;
        }

        if keyval == gtk4::gdk::Key::Escape {
            if !search_entry.text().is_empty() {
                search_entry.set_text("");
            } else if let Some(win) = list_state.view.root().and_downcast::<ApplicationWindow>()
            {
                win.close();
            }

            return glib::Propagation::Stop;
        }

        if keyval == gtk4::gdk::Key::Up || keyval == gtk4::gdk::Key::Down {
            controller.forward(&list_state.view);
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
