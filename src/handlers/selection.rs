use crate::domain::item::Item;
use gtk4::prelude::ObjectExt;
use gtk4::ListBox;

pub struct SelectionHandler;

impl SelectionHandler {
    /// 处理用户选择
    pub fn handle_selection(listbox: &ListBox) -> SelectionResult {
        // 从 main.rs::handle_selection 迁移
        if let Some(selected_row) = listbox.selected_row() {
            if let Some(item_ptr) = unsafe { selected_row.data::<Item>("item") } {
                let item = unsafe { &*item_ptr.as_ptr() };
                return SelectionResult::Selected(item.clone());
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
