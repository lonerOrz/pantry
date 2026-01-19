use gtk4::prelude::*;
use gtk4::{Label, Overlay};
use std::cell::RefCell;
use std::rc::Rc;

/// 搜索覆盖层组件
pub struct SearchOverlay {
    label: Label,
    visible: bool,
}

impl SearchOverlay {
    pub fn new(overlay: &Overlay) -> Self {
        let label = Label::new(None);
        label.add_css_class("app-notification");
        label.add_css_class("hidden");
        label.set_halign(gtk4::Align::Center);
        label.set_valign(gtk4::Align::End);
        label.set_margin_bottom(30);
        overlay.add_overlay(&label);

        SearchOverlay {
            label,
            visible: false,
        }
    }

    pub fn show(&self, text: &str) {
        self.label.set_text(text);
        self.label.remove_css_class("hidden");
    }

    pub fn hide(&self) {
        self.label.add_css_class("hidden");
    }

    pub fn is_visible(&self) -> bool {
        !self.label.has_css_class("hidden")
    }

    pub fn get_label(&self) -> &Label {
        &self.label
    }
}

/// 搜索状态管理
pub type SearchState = Rc<RefCell<String>>;
