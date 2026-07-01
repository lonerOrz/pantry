use crate::domain::item::Item;
use crate::ui::item_object::ItemObject;
use crate::ui::r#match::{fuzzy_match, relevance_score};
use gtk4::prelude::*;
use gtk4::{
    ApplicationWindow, Box as GtkBox, CustomFilter, CustomSorter, FilterChange, FilterListModel,
    Label, ListItem, ListView, Orientation, SignalListItemFactory, SingleSelection, SortListModel,
    SorterChange, gio,
};
use std::cmp::Ordering;
use std::fmt::Write;

use crate::ui::search::SearchState;

#[derive(Clone)]
pub struct ListState {
    store: gio::ListStore,
    filter: CustomFilter,
    sort_model: SortListModel,
    selection: SingleSelection,
    view: ListView,
    sorter: CustomSorter,
}

impl ListState {
    pub fn new(query_state: SearchState) -> Self {
        let store = gio::ListStore::new::<ItemObject>();
        let filter = build_filter(query_state.clone());
        let filter_model = FilterListModel::new(Some(store.clone()), Some(filter.clone()));
        let sorter = build_sorter(query_state.clone());
        let sort_model = SortListModel::new(Some(filter_model.clone()), Some(sorter.clone()));
        let selection = SingleSelection::new(Some(sort_model.clone()));
        selection.set_autoselect(false);
        selection.set_can_unselect(true);

        let factory = build_factory(query_state.clone());
        let view = ListView::new(Some(selection.clone()), Some(factory));
        view.set_margin_top(8);
        view.set_margin_bottom(8);
        view.set_margin_start(8);
        view.set_margin_end(8);
        view.add_css_class("pantry-list-view");

        Self {
            store,
            filter,
            sort_model,
            sorter,
            selection,
            view,
        }
    }

    pub fn append_item(&self, item: Item) {
        self.store.append(&ItemObject::new(item));
    }

    pub fn append_items(&self, items: &[Item]) {
        for item in items {
            self.append_item(item.clone());
        }
    }

    pub fn selected_item(&self) -> Option<Item> {
        self.selection
            .selected_item()
            .and_downcast::<ItemObject>()
            .and_then(|item_object| item_object.item())
    }

    pub fn select_first(&self) {
        if self.sort_model.n_items() == 0 {
            self.selection.set_selected(gtk4::INVALID_LIST_POSITION);
        } else {
            self.selection.set_selected(0);
        }
    }

    pub fn refresh_filter(&self) {
        self.filter.changed(FilterChange::Different);
        self.sorter.changed(SorterChange::Different);
    }

    pub fn view(&self) -> &ListView {
        &self.view
    }

    pub fn grab_focus(&self) {
        self.view.grab_focus();
    }

    pub fn n_items(&self) -> u32 {
        self.sort_model.n_items()
    }

    pub fn selected_index(&self) -> u32 {
        self.selection.selected()
    }

    pub fn set_selected(&self, index: u32) {
        self.selection.set_selected(index);
    }

    pub fn scroll_to(&self, index: u32) {
        self.view
            .scroll_to(index, gtk4::ListScrollFlags::FOCUS, None);
    }

    pub fn toggle_marked(&self, sorted_index: u32) -> bool {
        if let Some(obj) = self
            .sort_model
            .item(sorted_index)
            .and_downcast::<ItemObject>()
        {
            let new_marked = !obj.is_marked();
            obj.set_marked(new_marked);

            if let Some(index) = self.store.find(&obj) {
                self.store.remove(index);
                self.store.insert(index, &obj);
            }
            true
        } else {
            false
        }
    }

    pub fn forward_key(&self, controller: &gtk4::EventControllerKey) -> bool {
        controller.forward(&self.view)
    }

    pub fn close_window(&self, save_state: bool) {
        if let Some(win) = self
            .view
            .root()
            .and_then(|r| r.downcast::<ApplicationWindow>().ok())
        {
            if save_state {
                let (width, height) = win.default_size();
                let state = crate::window_state::WindowState {
                    width,
                    height,
                    maximized: win.is_maximized(),
                };
                state.save();
            }
            win.close();
        }
    }

    pub fn marked_values(&self) -> Vec<String> {
        let mut values = Vec::new();
        let n = self.store.n_items();
        for i in 0..n {
            if let Some(obj) = self.store.item(i).and_downcast::<ItemObject>()
                && obj.is_marked()
            {
                values.push(obj.value());
            }
        }
        values
    }

    pub fn connect_selection_changed<F>(&self, callback: F)
    where
        F: Fn() + 'static,
    {
        self.selection.connect_selection_changed(move |_, _, _| {
            callback();
        });
    }

    pub fn connect_items_changed<F>(&self, callback: F)
    where
        F: Fn(u32) + 'static,
    {
        self.sort_model
            .connect_items_changed(move |model, _, _, _| {
                callback(model.n_items());
            });
    }
}

fn build_filter(query_state: SearchState) -> CustomFilter {
    CustomFilter::new(move |obj| {
        let query = query_state.borrow();
        if query.is_empty() {
            return true;
        }

        let Some(item_object) = obj.downcast_ref::<ItemObject>() else {
            return false;
        };

        let query = query.to_lowercase();
        let search_text = item_object.search_text();
        search_text.contains(&query) || fuzzy_match(&search_text, &query)
    })
}

fn build_sorter(query_state: SearchState) -> CustomSorter {
    CustomSorter::new(move |obj1, obj2| {
        let query = query_state.borrow().clone();
        if query.is_empty() {
            return Ordering::Equal.into();
        }

        let item1 = obj1.downcast_ref::<ItemObject>();
        let item2 = obj2.downcast_ref::<ItemObject>();

        let score1 = item1
            .and_then(|i| relevance_score(&i.title(), &i.value(), &query))
            .unwrap_or(0);
        let score2 = item2
            .and_then(|i| relevance_score(&i.title(), &i.value(), &query))
            .unwrap_or(0);

        score2.cmp(&score1).into()
    })
}

fn build_factory(query_state: SearchState) -> SignalListItemFactory {
    let factory = SignalListItemFactory::new();

    factory.connect_setup(|_, obj| {
        let list_item = obj
            .downcast_ref::<ListItem>()
            .expect("factory setup object must be a ListItem");

        let row = GtkBox::new(Orientation::Vertical, 2);
        row.add_css_class("bookmark-row");

        let title_label = Label::new(None);
        title_label.set_xalign(0.0);
        title_label.set_use_markup(true);
        title_label.set_ellipsize(gtk4::pango::EllipsizeMode::End);
        title_label.add_css_class("bookmark-title");

        let value_label = Label::new(None);
        value_label.set_xalign(0.0);
        value_label.add_css_class("bookmark-value");
        value_label.set_ellipsize(gtk4::pango::EllipsizeMode::End);
        value_label.set_single_line_mode(true);

        row.append(&title_label);
        row.append(&value_label);
        list_item.set_child(Some(&row));
    });

    factory.connect_bind(move |_, obj| {
        let list_item = obj
            .downcast_ref::<ListItem>()
            .expect("factory bind object must be a ListItem");
        let Some(item_object) = list_item.item().and_downcast::<ItemObject>() else {
            return;
        };
        let Some(row) = list_item.child().and_downcast::<GtkBox>() else {
            return;
        };
        let Some(title_label) = row.first_child().and_downcast::<Label>() else {
            return;
        };
        let Some(value_label) = title_label.next_sibling().and_downcast::<Label>() else {
            return;
        };

        let marked = item_object.is_marked();

        let query = query_state.borrow();
        if query.is_empty() {
            let base = glib::markup_escape_text(&item_object.title());
            if marked {
                title_label.set_markup(&format!(
                    "<span foreground='#3584e4' weight='bold'>✓ </span>{}",
                    base
                ));
                row.add_css_class("marked-row");
            } else {
                title_label.set_markup(&base);
                row.remove_css_class("marked-row");
            }
        } else {
            let highlighted = highlight_title(&item_object.title(), &query);
            if marked {
                title_label.set_markup(&format!(
                    "<span foreground='#3584e4' weight='bold'>✓ </span>{}",
                    highlighted
                ));
                row.add_css_class("marked-row");
            } else {
                title_label.set_markup(&highlighted);
                row.remove_css_class("marked-row");
            }
        }
        value_label.set_label(&item_object.value());
    });

    factory.connect_unbind(|_, obj| {
        let Some(list_item) = obj.downcast_ref::<ListItem>() else {
            return;
        };
        let Some(row) = list_item.child().and_downcast::<GtkBox>() else {
            return;
        };
        let Some(title_label) = row.first_child().and_downcast::<Label>() else {
            return;
        };
        let Some(value_label) = title_label.next_sibling().and_downcast::<Label>() else {
            return;
        };

        title_label.set_label("");
        value_label.set_label("");
    });

    factory
}

fn highlight_title(title: &str, query: &str) -> String {
    let title_lower = title.to_lowercase();
    let query_lower = query.to_lowercase();

    if let Some(start) = title_lower.find(&query_lower) {
        let end = start + query.len();
        let before = &title[..start];
        let matched = &title[start..end];
        let after = &title[end..];
        return format!(
            "{}<span foreground='#3584e4' weight='bold'>{}</span>{}",
            glib::markup_escape_text(before),
            glib::markup_escape_text(matched),
            glib::markup_escape_text(after),
        );
    }

    if !fuzzy_match(&title_lower, &query_lower) {
        return glib::markup_escape_text(title).to_string();
    }

    let mut result = String::new();
    let mut qi = 0;
    let chars: Vec<char> = query_lower.chars().collect();

    for c in title.chars() {
        if qi < chars.len() && c.to_lowercase().next() == Some(chars[qi]) {
            let escaped = glib::markup_escape_text(&c.to_string());
            let _ = write!(
                result,
                "<span foreground='#3584e4' weight='bold'>{}</span>",
                escaped
            );
            qi += 1;
        } else {
            result.push_str(&glib::markup_escape_text(&c.to_string()));
        }
    }
    result
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn highlight_exact_match() {
        let result = highlight_title("Hello World", "Hello");
        assert!(result.contains("Hello"));
        assert!(result.contains("foreground"));
    }

    #[test]
    fn highlight_case_insensitive() {
        let result = highlight_title("Hello World", "hello");
        assert!(result.contains("Hello"));
    }

    #[test]
    fn highlight_no_match() {
        let result = highlight_title("Hello World", "xyz");
        assert!(!result.contains("foreground"));
    }

    #[test]
    fn highlight_fuzzy() {
        let result = highlight_title("fobar", "fo");
        assert!(result.contains("<span"));
    }
}
