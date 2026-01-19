use crate::domain::item::Item;

/// 过滤器
pub struct Filter {
    query: String,
}

impl Filter {
    pub fn new() -> Self {
        Filter {
            query: String::new(),
        }
    }

    pub fn set_query(&mut self, query: String) {
        self.query = query;
    }

    pub fn should_show(&self, item: &Item) -> bool {
        if self.query.is_empty() {
            return true;
        }

        let query_lower = self.query.to_lowercase();
        let title_lower = item.title.to_lowercase();
        let value_lower = item.value.to_lowercase();

        title_lower == query_lower
            || value_lower == query_lower
            || title_lower.contains(&query_lower)
            || value_lower.contains(&query_lower)
            || FuzzyMatcher::match_text(&title_lower, &query_lower)
            || FuzzyMatcher::match_text(&value_lower, &query_lower)
    }

    /// 为 ListBox 设置过滤函数
    pub fn setup_listbox_filter(
        listbox: &gtk4::ListBox,
        query_state: std::rc::Rc<std::cell::RefCell<String>>,
    ) {
        use crate::domain::item::Item;
        use gtk4::prelude::*;

        listbox.set_filter_func(Box::new(move |row: &gtk4::ListBoxRow| -> bool {
            let query = query_state.borrow();
            if query.is_empty() {
                return true;
            }
            if let Some(item_obj) = row.property::<Option<crate::app::item_object::ItemObject>>("item") {
                if let Some(item) = item_obj.item() {
                    let query_lower = query.to_lowercase();
                    let title_lower = item.title.to_lowercase();
                    let value_lower = item.value.to_lowercase();
                    title_lower == query_lower
                        || value_lower == query_lower
                        || title_lower.contains(&query_lower)
                        || value_lower.contains(&query_lower)
                        || FuzzyMatcher::match_text(&title_lower, &query_lower)
                        || FuzzyMatcher::match_text(&value_lower, &query_lower)
                } else {
                    false
                }
            } else {
                false
            }
        }));
    }
}

/// 模糊匹配器
pub struct FuzzyMatcher;

impl FuzzyMatcher {
    /// 模糊匹配文本
    pub fn match_text(text: &str, pattern: &str) -> bool {
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

    /// 匹配项目的标题或值
    pub fn match_item(item: &Item, pattern: &str) -> bool {
        let title_match = Self::match_text(&item.title.to_lowercase(), &pattern.to_lowercase());
        let value_match = Self::match_text(&item.value.to_lowercase(), &pattern.to_lowercase());
        title_match || value_match
    }
}
