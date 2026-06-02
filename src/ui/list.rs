use crate::app::item_object::ItemObject;
use crate::domain::item::Item;
use gtk4::prelude::*;
use gtk4::{
    Box as GtkBox, CustomFilter, FilterChange, FilterListModel, Label, ListItem, ListView,
    Orientation, SignalListItemFactory, SingleSelection, gio,
};

#[derive(Clone)]
pub struct ListState {
    pub store: gio::ListStore,
    pub filter: CustomFilter,
    pub filter_model: FilterListModel,
    pub selection: SingleSelection,
    pub view: ListView,
}

impl ListState {
    pub fn new(query_state: crate::ui::search::SearchState) -> Self {
        let store = gio::ListStore::new::<ItemObject>();
        let filter = build_filter(query_state);
        let filter_model = FilterListModel::new(Some(store.clone()), Some(filter.clone()));
        let selection = SingleSelection::new(Some(filter_model.clone()));
        selection.set_autoselect(false);
        selection.set_can_unselect(true);

        let factory = build_factory();
        let view = ListView::new(Some(selection.clone()), Some(factory));
        view.set_margin_top(20);
        view.set_margin_bottom(20);
        view.set_margin_start(20);
        view.set_margin_end(20);
        view.add_css_class("pantry-list-view");

        Self {
            store,
            filter,
            filter_model,
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
        if self.filter_model.n_items() == 0 {
            self.selection.set_selected(gtk4::INVALID_LIST_POSITION);
        } else {
            self.selection.set_selected(0);
            self.view.grab_focus();
        }
    }

    pub fn refresh_filter(&self) {
        self.filter.changed(FilterChange::Different);
    }
}

fn build_filter(query_state: crate::ui::search::SearchState) -> CustomFilter {
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

fn build_factory() -> SignalListItemFactory {
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

        let value_label = Label::new(None);
        value_label.set_xalign(0.0);
        value_label.add_css_class("dim-label");
        value_label.set_ellipsize(gtk4::pango::EllipsizeMode::End);
        value_label.set_single_line_mode(true);

        row.append(&title_label);
        row.append(&value_label);
        list_item.set_child(Some(&row));
    });

    factory.connect_bind(|_, obj| {
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

        title_label.set_label(&glib::markup_escape_text(&item_object.title()));
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

fn fuzzy_match(text: &str, pattern: &str) -> bool {
    let mut pattern_chars = pattern.chars();
    let Some(mut expected) = pattern_chars.next() else {
        return true;
    };

    for c in text.chars() {
        if c == expected {
            match pattern_chars.next() {
                Some(next) => expected = next,
                None => return true,
            }
        }
    }

    false
}
