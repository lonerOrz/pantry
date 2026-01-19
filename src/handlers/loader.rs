use crate::config::Config;
use crate::domain::item::Item;
use gtk4::ListBox;

/// 项目加载配置
pub struct LoadConfig {
    pub category_filter: Option<String>,
    pub display_arg: Option<String>,
    pub config_path: String,
}

/// 项目加载器
pub struct ItemLoader {
    config_loader: Box<dyn ConfigLoaderTrait>,
    executor: Box<dyn ExecutorTrait>,
}

impl ItemLoader {
    pub fn new(
        config_loader: Box<dyn ConfigLoaderTrait>,
        executor: Box<dyn ExecutorTrait>,
    ) -> Self {
        ItemLoader {
            config_loader,
            executor,
        }
    }

    /// 从配置加载项目
    pub async fn load_from_config(&self, _config: &LoadConfig) -> Result<Vec<Item>, LoadError> {
        // 从 main.rs::load_items_from_config 迁移
        todo!()
    }

    /// 将项目添加到 ListBox
    pub fn add_to_listbox(&self, _listbox: &ListBox, _items: &[Item]) {
        // 从 main.rs::add_item_to_ui 迁移
        todo!()
    }
}

#[derive(Debug)]
pub enum LoadError {
    ConfigError(crate::config::loader::ConfigError),
    ExecutionError(crate::handlers::executor::ExecutionError),
    ProcessingError(String),
}

// Traits 用于依赖注入和测试
pub trait ConfigLoaderTrait {
    fn load(&self, path: &str) -> Result<Config, crate::config::loader::ConfigError>;
}

pub trait ExecutorTrait {
    fn execute(&self, command: &str) -> Result<String, crate::handlers::executor::ExecutionError>;
}
