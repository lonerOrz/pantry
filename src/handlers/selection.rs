use crate::domain::item::Item;
use gtk4::prelude::ObjectExt;
use gtk4::ListBox;

pub struct SelectionHandler;

impl SelectionHandler {
    /// 处理用户选择
    pub fn handle_selection(listbox: &ListBox) -> SelectionResult {
        // 从 main.rs::handle_selection 迁移
        if let Some(selected_row) = listbox.selected_row() {
            if let Some(item_obj_ptr) =
                unsafe { selected_row.data::<crate::app::item_object::ItemObject>("item") }
            {
                let item_obj = unsafe { &*item_obj_ptr.as_ptr() };
                if let Some(item) = item_obj.item() {
                    return SelectionResult::Selected(item);
                }
            }
        }
        SelectionResult::None
    }
}

pub enum SelectionResult {
    Selected(Item),
    None,
    Error(String),
}
