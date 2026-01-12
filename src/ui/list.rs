use gtk4::{
    ListBox, ListBoxRow, Orientation, Box as GtkBox, Label,
    SelectionMode,
};
use gtk4::prelude::*;

pub fn create_listbox() -> ListBox {
    ListBox::builder()
        .selection_mode(SelectionMode::Single)
        .margin_top(20)
        .margin_bottom(20)
        .margin_start(20)
        .margin_end(20)
        .activate_on_single_click(true)
        .build()
}

pub fn create_list_item(title: &str, value: &str) -> ListBoxRow {
    let row = ListBoxRow::new();
    row.add_css_class("bookmark-row");
    let vbox = GtkBox::new(Orientation::Vertical, 2);
    let title_label = Label::new(Some(title));
    title_label.set_xalign(0.0);
    title_label.set_use_markup(true);
    title_label.set_ellipsize(gtk4::pango::EllipsizeMode::End);
    let value_label = Label::new(Some(value));
    value_label.set_xalign(0.0);
    value_label.add_css_class("dim-label");
    value_label.set_ellipsize(gtk4::pango::EllipsizeMode::End);
    value_label.set_single_line_mode(true);
    vbox.append(&title_label);
    vbox.append(&value_label);
    row.set_child(Some(&vbox));
    row
}