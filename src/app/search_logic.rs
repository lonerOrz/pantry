use gtk4::{prelude::*, ListBox, ListBoxRow};

pub struct SearchLogic;

impl SearchLogic {
    pub fn setup_filter_func(listbox: &ListBox, query_state: crate::ui::search::SearchState) {
        listbox.set_filter_func(Box::new(move |row: &ListBoxRow| -> bool {
            let query = query_state.borrow();
            if query.is_empty() {
                return true;
            }
            if let Some(item_obj_ptr) =
                unsafe { row.data::<crate::app::item_object::ItemObject>("item") }
            {
                let item_obj = unsafe { &*item_obj_ptr.as_ptr() };
                if let Some(item) = item_obj.item() {
                    let query_lower = query.to_lowercase();
                    let title_lower = item.title.to_lowercase();
                    let value_lower = item.value.to_lowercase();
                    title_lower == query_lower
                        || value_lower == query_lower
                        || title_lower.contains(&query_lower)
                        || value_lower.contains(&query_lower)
                        || fuzzy_match(&title_lower, &query_lower)
                        || fuzzy_match(&value_lower, &query_lower)
                } else {
                    false
                }
            } else {
                false
            }
        }));
    }
}

fn fuzzy_match(text: &str, pattern: &str) -> bool {
    let text_chars: Vec<char> = text.chars().collect();
    let pattern_chars: Vec<char> = pattern.chars().collect();
    let mut text_idx = 0;
    let mut pattern_idx = 0;
    while text_idx < text_chars.len() && pattern_idx < pattern_chars.len() {
        if text_chars[text_idx] == pattern_chars[pattern_idx] {
            pattern_idx += 1;
        }
        text_idx += 1;
    }
    pattern_idx == pattern_chars.len()
}
