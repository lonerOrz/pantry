use std::cell::RefCell;
use std::rc::Rc;

/// Search state management
pub type SearchState = Rc<RefCell<String>>;
