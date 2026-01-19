pub mod loader;
pub mod parser;
pub mod resolver;

pub use parser::{Config, DisplayMode, SourceMode};
pub use resolver::{get_config_display_mode, resolve_display_mode};
