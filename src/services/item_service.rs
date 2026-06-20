use crate::domain::item::Item;

/// Process items for display (e.g., expand directories in picture mode)
pub fn process_items_for_display(items: Vec<Item>) -> Vec<Item> {
    use rayon::prelude::*;
    items
        .par_iter()
        .flat_map(crate::services::expansion::ItemProcessor::process_for_display)
        .collect()
}
