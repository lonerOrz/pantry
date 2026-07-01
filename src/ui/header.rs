use gtk4::{AboutDialog, ApplicationWindow, Button, HeaderBar, Label, SearchEntry, prelude::*};

use crate::app::preview_manager::PreviewUpdater;
use crate::ui::list::ListState;
use crate::ui::preview;

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

pub fn connect_search_changed(
    search_entry: &SearchEntry,
    list_state: &ListState,
    query_state: &crate::ui::search::SearchState,
    preview_area_rc_opt: &Option<std::rc::Rc<std::cell::RefCell<preview::PreviewArea>>>,
    preview_manager: &std::rc::Rc<std::cell::RefCell<dyn PreviewUpdater>>,
) {
    let list_state_clone = list_state.clone();
    let query_state_clone = query_state.clone();
    let preview_area_rc_opt_clone = preview_area_rc_opt.clone();
    let preview_manager_clone = preview_manager.clone();

    search_entry.connect_search_changed(move |entry| {
        {
            let mut query = query_state_clone.borrow_mut();
            query.clear();
            query.push_str(&entry.text());
        }
        list_state_clone.refresh_filter();
        list_state_clone.select_first();
        preview_manager_clone
            .borrow()
            .update_preview(&list_state_clone, &preview_area_rc_opt_clone);
    });
}
