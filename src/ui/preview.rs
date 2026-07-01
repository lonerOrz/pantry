use crate::domain::item::Item;
use crate::services::preview::PreviewPayload;
use gtk4::prelude::*;
use gtk4::{Align, Grid, Label, Picture, ScrolledWindow, TextView};

#[derive(Clone)]
pub struct PreviewArea {
    pub container: Grid,
    content_scrolled: ScrolledWindow,
    title_label: Label,
    category_label: Label,
    path_label: Label,
    details_scrolled: ScrolledWindow,
}

impl PreviewArea {
    pub fn new() -> Self {
        let title_label = Label::new(None);
        title_label.set_halign(Align::Start);
        title_label.add_css_class("preview-title");

        let category_label = Label::new(None);
        category_label.set_halign(Align::Start);
        category_label.add_css_class("preview-category");

        let path_label = Label::new(None);
        path_label.set_halign(Align::Start);
        path_label.set_ellipsize(gtk4::pango::EllipsizeMode::End);
        path_label.add_css_class("preview-path");

        let details_box = gtk4::Box::new(gtk4::Orientation::Vertical, 4);
        details_box.append(&title_label);
        details_box.append(&category_label);
        details_box.append(&path_label);
        details_box.add_css_class("preview-details-box");

        let details_scrolled = ScrolledWindow::new();
        details_scrolled.set_child(Some(&details_box));
        details_scrolled.set_vexpand(false);
        details_scrolled.set_hscrollbar_policy(gtk4::PolicyType::Automatic);
        details_scrolled.set_vscrollbar_policy(gtk4::PolicyType::Automatic);
        details_scrolled.set_size_request(-1, 100);
        details_scrolled.add_css_class("preview-details-scrolled");

        let content_scrolled = ScrolledWindow::new();
        content_scrolled.set_vexpand(true);
        content_scrolled.set_hexpand(true);
        content_scrolled.set_hscrollbar_policy(gtk4::PolicyType::Automatic);
        content_scrolled.set_vscrollbar_policy(gtk4::PolicyType::Automatic);
        content_scrolled.add_css_class("preview-content-scrolled");

        let separator = gtk4::Separator::new(gtk4::Orientation::Horizontal);
        separator.add_css_class("preview-separator");

        let container = Grid::new();
        container.set_row_homogeneous(false);
        container.set_column_homogeneous(false);
        container.set_vexpand(true);
        container.set_row_spacing(0);

        container.attach(&content_scrolled, 0, 0, 1, 1);
        container.attach(&separator, 0, 1, 1, 1);
        container.attach(&details_scrolled, 0, 2, 1, 1);

        Self {
            container,
            content_scrolled,
            title_label,
            category_label,
            path_label,
            details_scrolled,
        }
    }

    pub fn render(&self, payload: PreviewPayload, item: &Item) {
        if matches!(item.display, crate::domain::DisplayMode::Picture) {
            self.details_scrolled.set_visible(true);
            self.title_label
                .set_markup(&format!("<b>{}</b>", glib::markup_escape_text(&item.title)));
            self.category_label
                .set_text(&format!("Category: {}", item.category));
            self.path_label.set_text(&format!("Path: {}", item.value));
        } else {
            self.details_scrolled.set_visible(false);
        }

        self.content_scrolled.set_child(None::<&gtk4::Widget>);

        match payload {
            PreviewPayload::Text(text) => {
                let text_view = create_text_view(&text);
                let scrolled = create_text_scrolled(&text_view);
                self.set_content(&scrolled);
            }
            PreviewPayload::Image {
                bytes,
                width,
                height,
            } => {
                let gbytes = glib::Bytes::from(&bytes[..]);
                let texture = gtk4::gdk::MemoryTexture::new(
                    width,
                    height,
                    gtk4::gdk::MemoryFormat::R8g8b8a8,
                    &gbytes,
                    (width * 4) as usize,
                );
                let picture = Picture::for_paintable(&texture);
                picture.set_halign(Align::Center);
                picture.set_valign(Align::Center);
                picture.set_hexpand(true);
                picture.set_vexpand(true);
                self.set_content(&picture);
            }
            PreviewPayload::Error(err) => {
                let label = Label::new(Some(&err));
                label.set_halign(Align::Center);
                label.set_valign(Align::Center);
                label.set_hexpand(true);
                label.set_vexpand(true);
                label.add_css_class("preview-error-box");
                self.set_content(&label);
            }
        }
    }

    fn set_content<W: IsA<gtk4::Widget>>(&self, widget: &W) {
        self.content_scrolled.set_child(Some(widget));
    }
}

fn create_text_view(text: &str) -> TextView {
    let text_view = TextView::new();
    text_view.set_editable(false);
    text_view.set_cursor_visible(false);
    text_view.set_wrap_mode(gtk4::WrapMode::Word);
    text_view.set_left_margin(10);
    text_view.set_right_margin(10);
    text_view.set_top_margin(10);
    text_view.set_bottom_margin(10);
    text_view.buffer().set_text(text);
    text_view.set_hexpand(true);
    text_view.set_vexpand(true);
    text_view
}

fn create_text_scrolled(text_view: &TextView) -> ScrolledWindow {
    let scrolled = ScrolledWindow::new();
    scrolled.set_child(Some(text_view));
    scrolled.set_hexpand(true);
    scrolled.set_vexpand(true);
    scrolled.set_hscrollbar_policy(gtk4::PolicyType::Automatic);
    scrolled.set_vscrollbar_policy(gtk4::PolicyType::Automatic);
    scrolled.add_css_class("preview-text-box");
    scrolled
}
