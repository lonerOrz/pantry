pub mod parser;
pub mod resolver;

pub use crate::domain::{DisplayMode, SourceMode};
pub use parser::{Category, Config};
pub use resolver::{get_config_display_mode, resolve_display_mode};
