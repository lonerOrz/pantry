use crate::ui::list::ListState;

pub struct SearchLogic;

impl SearchLogic {
    pub fn refresh_filter(list_state: &ListState) {
        list_state.refresh_filter();
    }
}
