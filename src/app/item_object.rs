use glib::{subclass::prelude::*, Object};
use gtk4::glib;
use gtk4::prelude::ObjectExt;
use gtk4::ListBoxRow;

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

    /// Safe wrapper to get ItemObject from ListBoxRow
    /// Returns None if no item is attached
    pub fn from_row(row: &ListBoxRow) -> Option<ItemObject> {
        unsafe {
            row.data::<ItemObject>("item")
                .map(|ptr| ptr.as_ref().clone())
        }
    }

    /// Attach ItemObject to ListBoxRow
    pub fn attach_to_row(&self, row: &ListBoxRow) {
        unsafe {
            row.set_data("item", self.clone());
        }
    }
}
