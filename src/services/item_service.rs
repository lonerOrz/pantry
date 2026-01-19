use gtk4::prelude::ObjectExt;
use gtk4::prelude::WidgetExt;
use gtk4::ListBox;
use crate::domain::item::Item;

/// Service for managing item operations
pub struct ItemService;

impl ItemService {
    /// Process items for display (e.g., expand directories in picture mode)
    pub fn process_items_for_display(items: Vec<Item>) -> Vec<Item> {
        use rayon::prelude::*;
        items
            .par_iter()
            .flat_map(|item| crate::domain::item::ItemProcessor::process_for_display(item))
            .collect()
    }

    /// Add processed items to the UI listbox
    pub fn add_items_to_listbox(listbox: &ListBox, items: &[Item]) {
        for item in items {
            let row = crate::ui::list::create_list_item(&item.title, &item.value);
            let item_obj = crate::app::item_object::ItemObject::new(item.clone());
            unsafe {
                row.set_data("item", item_obj);
            }
            listbox.append(&row);
        }
    }

    /// Select the first item in the listbox
    pub fn select_first_item(listbox: &ListBox) {
        if let Some(first_row) = listbox.row_at_index(0) {
            listbox.select_row(Some(&first_row));
            first_row.grab_focus();
        }
    }
}