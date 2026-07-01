use glib::{Object, subclass::prelude::*};
use gtk4::glib;

#[derive(Default)]
pub struct ItemData {
    item: std::cell::RefCell<Option<crate::domain::item::Item>>,
    search_text: std::cell::RefCell<String>,
    marked: std::cell::Cell<bool>,
}

#[glib::object_subclass]
impl ObjectSubclass for ItemData {
    const NAME: &'static str = "PantryItemObject";
    type Type = ItemObject;
}

impl ObjectImpl for ItemData {}

glib::wrapper! {
    pub struct ItemObject(ObjectSubclass<ItemData>);
}

impl ItemObject {
    pub fn new(item: crate::domain::item::Item) -> Self {
        let obj: Self = Object::new();
        obj.set_item(item);
        obj
    }

    pub fn item(&self) -> Option<crate::domain::item::Item> {
        self.imp().item.borrow().clone()
    }

    pub fn set_item(&self, item: crate::domain::item::Item) {
        let search_text =
            format!("{}\n{}\n{}", item.title, item.value, item.category).to_lowercase();
        self.imp().search_text.replace(search_text);
        self.imp().item.replace(Some(item));
    }

    pub fn title(&self) -> String {
        self.item().map(|item| item.title).unwrap_or_default()
    }

    pub fn value(&self) -> String {
        self.item().map(|item| item.value).unwrap_or_default()
    }

    pub fn search_text(&self) -> String {
        self.imp().search_text.borrow().clone()
    }

    pub fn is_marked(&self) -> bool {
        self.imp().marked.get()
    }

    pub fn set_marked(&self, marked: bool) {
        self.imp().marked.set(marked);
    }
}
