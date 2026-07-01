use gtk4::{AboutDialog, ApplicationWindow, Button, HeaderBar, Label, SearchEntry, prelude::*};
use std::cell::RefCell;
use std::rc::Rc;

use crate::ui::list::ListState;

pub fn build_header_bar() -> (HeaderBar, SearchEntry, Button) {
    let header_bar = HeaderBar::new();
    header_bar.set_show_title_buttons(true);

    let title_label = Label::new(Some("pantry"));
    title_label.add_css_class("pantry-title-label");
    header_bar.pack_start(&title_label);

    let search_entry = SearchEntry::new();
    search_entry.add_css_class("pantry-search-entry");
    header_bar.set_title_widget(Some(&search_entry));

    let menu_button = Button::from_icon_name("open-menu-symbolic");
    menu_button.add_css_class("flat");
    header_bar.pack_end(&menu_button);

    (header_bar, search_entry, menu_button)
}

pub fn connect_about_dialog(window: &ApplicationWindow, menu_button: &Button) {
    let window_clone = window.clone();
    menu_button.connect_clicked(move |_| {
        let about = AboutDialog::new();
        about.set_transient_for(Some(&window_clone));
        about.set_modal(true);
        about.set_program_name(Some("pantry"));
        about.set_version(Some(env!("CARGO_PKG_VERSION")));
        about.set_copyright(Some("© 2025, lonerorz"));
        about.set_comments(Some(
            "A generic selector tool with text and image preview modes",
        ));
        about.set_website(Some("https://github.com/lonerOrz/pantry"));
        about.set_website_label("GitHub Repository");
        about.set_license(Some(include_str!("../../LICENSE")));
        about.set_authors(&["lonerorz <2788892716@qq.com>"]);
        about.set_artists(&["lonerorz"]);
        about.set_logo_icon_name(Some("system-search-symbolic"));
        about.present();
    });
}

pub fn connect_search_changed<F>(
    search_entry: &SearchEntry,
    list_state: &ListState,
    query_state: &crate::ui::search::SearchState,
    on_search_changed: F,
) where
    F: Fn() + 'static,
{
    let list_state_clone = list_state.clone();
    let query_state_clone = query_state.clone();
    let on_search_changed = Rc::new(on_search_changed);
    let debounce_timeout_id: Rc<RefCell<Option<glib::SourceId>>> = Rc::new(RefCell::new(None));

    search_entry.connect_search_changed(move |entry| {
        let list_state_inner = list_state_clone.clone();
        let query_state_inner = query_state_clone.clone();
        let on_search_changed_inner = on_search_changed.clone();
        let debounce_timeout_inner = debounce_timeout_id.clone();
        let query_text = entry.text().to_string();

        if let Some(old_id) = debounce_timeout_inner.borrow_mut().take() {
            old_id.remove();
        }

        let new_id = glib::timeout_add_local(
            std::time::Duration::from_millis(80),
            move || {
                debounce_timeout_inner.borrow_mut().take();

                {
                    let mut query = query_state_inner.borrow_mut();
                    query.clear();
                    query.push_str(&query_text);
                }

                list_state_inner.refresh_filter();
                list_state_inner.select_first();
                on_search_changed_inner();

                glib::ControlFlow::Break
            },
        );

        debounce_timeout_id.borrow_mut().replace(new_id);
    });
}
