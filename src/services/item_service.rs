use crate::domain::item::Item;
use crate::ui::list::ListState;

/// Service for managing item operations
pub struct ItemService;

impl ItemService {
    /// Process items for display (e.g., expand directories in picture mode)
    pub fn process_items_for_display(items: Vec<Item>) -> Vec<Item> {
        use rayon::prelude::*;
        items
            .par_iter()
            .flat_map(crate::services::expansion::ItemProcessor::process_for_display)
            .collect()
    }

    /// Add processed items to the UI model
    pub fn add_items_to_list(list_state: &ListState, items: &[Item]) {
        list_state.append_items(items);
    }

    /// Select the first visible item in the list
    pub fn select_first_item(list_state: &ListState) {
        list_state.select_first();
    }
}
