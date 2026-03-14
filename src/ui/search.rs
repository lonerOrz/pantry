use std::cell::RefCell;
use std::rc::Rc;

/// 搜索状态管理
pub type SearchState = Rc<RefCell<String>>;
