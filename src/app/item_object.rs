use glib::{subclass::prelude::*, Object};
use gtk4::glib;

#[derive(Default)]
pub struct ItemData {
    pub item: std::cell::RefCell<Option<crate::domain::item::Item>>,
}

#[glib::object_subclass]
impl ObjectSubclass for ItemData {
    const NAME: &'static str = "ItemData";
    type Type = ItemObject;
}

impl ObjectImpl for ItemData {}

glib::wrapper! {
    pub struct ItemObject(ObjectSubclass<ItemData>);
}

impl ItemObject {
    pub fn new(item: crate::domain::item::Item) -> Self {
        let obj: Self = Object::new();
        obj.imp().item.replace(Some(item));
        obj
    }

    pub fn item(&self) -> Option<crate::domain::item::Item> {
        self.imp().item.borrow().clone()
    }

    pub fn set_item(&self, item: crate::domain::item::Item) {
        self.imp().item.replace(Some(item));
    }
}
