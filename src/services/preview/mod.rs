pub mod decoder;
pub mod detector;
pub mod video;

use crate::cache::{CacheAdapter, CacheManager};
use crate::domain::item::Item;
use crate::services::process::CommandExecutor;
use std::path::Path;
use std::sync::Arc;

pub use decoder::{GdkPixbufDecoder, ImageDecoder};

#[derive(Debug, Clone)]
pub enum PreviewPayload {
    Text(String),
    Image {
        bytes: Arc<Vec<u8>>,
        width: i32,
        height: i32,
    },
    Error(String),
}

pub type ProdPreviewService =
    PreviewService<CacheManager, crate::services::process::ShellExec, GdkPixbufDecoder>;

pub fn create_prod_preview_service() -> ProdPreviewService {
    ProdPreviewService::new(
        CacheManager::new(),
        crate::services::process::ShellExec,
        GdkPixbufDecoder,
    )
}

#[derive(Clone)]
pub struct PreviewService<
    C: CacheAdapter + Clone,
    E: CommandExecutor + Clone,
    D: ImageDecoder + Clone,
> {
    cache: C,
    executor: E,
    decoder: D,
}

impl<C: CacheAdapter + Clone, E: CommandExecutor + Clone, D: ImageDecoder + Clone>
    PreviewService<C, E, D>
{
    pub fn new(cache: C, executor: E, decoder: D) -> Self {
        Self {
            cache,
            executor,
            decoder,
        }
    }

    fn load_valid_cache(&self, category: &str, path: &Path) -> Option<PreviewPayload> {
        if !path.exists() || !path.is_file() {
            return None;
        }
        let cache_path = self.cache.get_cache_path(category, path);
        if self.cache.is_cache_valid(&cache_path, path)
            && let Some((bytes, w, h)) = self.cache.load_raw_cache(&cache_path)
        {
            return Some(PreviewPayload::Image {
                bytes: Arc::new(bytes),
                width: w,
                height: h,
            });
        }
        None
    }

    pub fn try_cache(&self, item: &Item) -> Option<PreviewPayload> {
        if item.preview_template.is_some()
            || matches!(item.source, crate::domain::SourceMode::Dynamic)
        {
            return None;
        }
        if !matches!(item.display, crate::domain::DisplayMode::Picture) {
            return None;
        }
        let expanded_path = crate::utils::expand_tilde(&item.value);
        if video::is_video(&expanded_path) {
            return None;
        }
        self.load_valid_cache(&item.category, &expanded_path)
    }

    pub fn resolve_payload(&self, item: &Item) -> PreviewPayload {
        if item.preview_template.is_some()
            || matches!(item.source, crate::domain::SourceMode::Dynamic)
        {
            return self.resolve_dynamic(item);
        }

        match item.display {
            crate::domain::DisplayMode::Text => PreviewPayload::Text(item.value.clone()),
            crate::domain::DisplayMode::Picture => self.resolve_image(item),
        }
    }

    fn resolve_image(&self, item: &Item) -> PreviewPayload {
        let expanded_path = crate::utils::expand_tilde(&item.value);
        if !expanded_path.exists() || !expanded_path.is_file() {
            return PreviewPayload::Text(item.value.clone());
        }

        if let Some(payload) = self.load_valid_cache(&item.category, &expanded_path) {
            return payload;
        }

        let cache_path = self.cache.get_cache_path(&item.category, &expanded_path);

        if video::is_video(&expanded_path) {
            video::generate_thumbnail(
                &expanded_path,
                &cache_path,
                &self.cache,
                &self.decoder,
                &self.executor,
            )
        } else {
            let max_w = crate::constants::IMAGE_PREVIEW_WIDTH;
            let max_h = crate::constants::IMAGE_PREVIEW_HEIGHT;

            if let Some((bytes, w, h)) = self.decoder.load_from_path(&expanded_path, max_w, max_h) {
                let _ = self.cache.save_raw_cache(&cache_path, &bytes, w, h);
                PreviewPayload::Image {
                    bytes: Arc::new(bytes),
                    width: w,
                    height: h,
                }
            } else {
                PreviewPayload::Error("Failed to decode image".to_string())
            }
        }
    }

    fn resolve_dynamic(&self, item: &Item) -> PreviewPayload {
        let preview_cmd = if let Some(ref template) = item.preview_template {
            template.replace("{}", &item.value)
        } else {
            format!("cliphist decode {}", item.value)
        };

        match self.executor.execute("sh", &["-c", &preview_cmd]) {
            Ok(output) if output.success => {
                if detector::is_binary(&output.stdout) {
                    use std::io::Write;
                    use tempfile::NamedTempFile;

                    let mut temp_file = match NamedTempFile::new() {
                        Ok(tf) => tf,
                        Err(e) => {
                            return PreviewPayload::Error(format!("Tempfile error: {}", e));
                        }
                    };

                    if temp_file.write_all(&output.stdout).is_ok() {
                        let max_w = crate::constants::IMAGE_PREVIEW_WIDTH;
                        let max_h = crate::constants::IMAGE_PREVIEW_HEIGHT;
                        if let Some((bytes, w, h)) =
                            self.decoder.load_from_path(temp_file.path(), max_w, max_h)
                        {
                            PreviewPayload::Image {
                                bytes: Arc::new(bytes),
                                width: w,
                                height: h,
                            }
                        } else {
                            PreviewPayload::Error(
                                "Failed to decode dynamic binary image".to_string(),
                            )
                        }
                    } else {
                        PreviewPayload::Error("Failed to write dynamic temp file".to_string())
                    }
                } else {
                    PreviewPayload::Text(String::from_utf8_lossy(&output.stdout).to_string())
                }
            }
            _ => PreviewPayload::Text(item.value.clone()),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::domain::{DisplayMode, SourceMode};
    use crate::services::process::MockExec;
    use std::collections::HashMap;
    use std::io;
    use std::path::PathBuf;
    use std::sync::RwLock;

    type CacheEntry = (Vec<u8>, i32, i32);

    struct MockCache {
        valid_entries: HashMap<PathBuf, bool>,
        stored: RwLock<HashMap<PathBuf, CacheEntry>>,
    }

    impl Clone for MockCache {
        fn clone(&self) -> Self {
            Self {
                valid_entries: self.valid_entries.clone(),
                stored: RwLock::new(self.stored.read().unwrap().clone()),
            }
        }
    }

    impl MockCache {
        fn new() -> Self {
            Self {
                valid_entries: HashMap::new(),
                stored: RwLock::new(HashMap::new()),
            }
        }

        fn with_valid(mut self, cache_path: PathBuf, data: Vec<u8>, w: i32, h: i32) -> Self {
            self.valid_entries.insert(cache_path.clone(), true);
            self.stored
                .write()
                .unwrap()
                .insert(cache_path, (data, w, h));
            self
        }
    }

    impl CacheAdapter for MockCache {
        fn get_cache_path(&self, category: &str, original_path: &Path) -> PathBuf {
            PathBuf::from(format!(
                "mock_cache/{}_{}",
                category,
                original_path
                    .file_name()
                    .unwrap_or_default()
                    .to_string_lossy()
            ))
        }

        fn is_cache_valid(&self, cache_path: &Path, _original_path: &Path) -> bool {
            *self.valid_entries.get(cache_path).unwrap_or(&false)
        }

        fn save_raw_cache(
            &self,
            path: &Path,
            raw_data: &[u8],
            width: i32,
            height: i32,
        ) -> io::Result<()> {
            self.stored
                .write()
                .unwrap()
                .insert(path.to_path_buf(), (raw_data.to_vec(), width, height));
            Ok(())
        }

        fn load_raw_cache(&self, path: &Path) -> Option<(Vec<u8>, i32, i32)> {
            self.stored.read().unwrap().get(path).cloned()
        }
    }

    #[derive(Clone)]
    struct MockDecoder {
        result: Option<(Vec<u8>, i32, i32)>,
    }

    impl MockDecoder {
        fn new() -> Self {
            Self { result: None }
        }

        fn with_result(mut self, data: Vec<u8>, w: i32, h: i32) -> Self {
            self.result = Some((data, w, h));
            self
        }
    }

    impl ImageDecoder for MockDecoder {
        fn load_from_path(
            &self,
            _path: &Path,
            _max_width: i32,
            _max_height: i32,
        ) -> Option<(Vec<u8>, i32, i32)> {
            self.result.clone()
        }
    }

    fn text_item(value: &str) -> Item {
        Item {
            title: "test".into(),
            value: value.into(),
            category: "cat".into(),
            display: DisplayMode::Text,
            source: SourceMode::Config,
            preview_template: None,
        }
    }

    fn dynamic_item(value: &str) -> Item {
        Item {
            title: "test".into(),
            value: value.into(),
            category: "cat".into(),
            display: DisplayMode::Text,
            source: SourceMode::Dynamic,
            preview_template: None,
        }
    }

    fn dynamic_item_with_template(value: &str, template: &str) -> Item {
        Item {
            title: "test".into(),
            value: value.into(),
            category: "cat".into(),
            display: DisplayMode::Text,
            source: SourceMode::Dynamic,
            preview_template: Some(template.into()),
        }
    }

    fn picture_item(path: &str) -> Item {
        Item {
            title: "test".into(),
            value: path.into(),
            category: "cat".into(),
            display: DisplayMode::Picture,
            source: SourceMode::Config,
            preview_template: None,
        }
    }

    #[test]
    fn text_mode_returns_value() {
        let svc = PreviewService::new(MockCache::new(), MockExec::new(), MockDecoder::new());
        let item = text_item("hello world");
        assert!(matches!(
            svc.resolve_payload(&item),
            PreviewPayload::Text(ref s) if s == "hello world"
        ));
    }

    #[test]
    fn dynamic_text_stdout() {
        let exec = MockExec::new().push_ok(true, b"clipboard text".to_vec());
        let svc = PreviewService::new(MockCache::new(), exec, MockDecoder::new());
        let item = dynamic_item("id123");
        assert!(matches!(
            svc.resolve_payload(&item),
            PreviewPayload::Text(ref s) if s == "clipboard text"
        ));
    }

    #[test]
    fn dynamic_command_failure_returns_value() {
        let exec = MockExec::new().push_err(io::Error::new(io::ErrorKind::NotFound, "no such cmd"));
        let svc = PreviewService::new(MockCache::new(), exec, MockDecoder::new());
        let item = dynamic_item("fallback");
        assert!(matches!(
            svc.resolve_payload(&item),
            PreviewPayload::Text(ref s) if s == "fallback"
        ));
    }

    #[test]
    fn dynamic_nonzero_exit_returns_value() {
        let exec = MockExec::new().push_ok(false, Vec::new());
        let svc = PreviewService::new(MockCache::new(), exec, MockDecoder::new());
        let item = dynamic_item("val");
        assert!(matches!(
            svc.resolve_payload(&item),
            PreviewPayload::Text(ref s) if s == "val"
        ));
    }

    #[test]
    fn dynamic_binary_stdout_decoded() {
        let exec = MockExec::new().push_ok(true, vec![0x00, 0x01, 0x02]);
        let decoder = MockDecoder::new().with_result(vec![255; 400], 20, 20);
        let svc = PreviewService::new(MockCache::new(), exec, decoder);
        let item = dynamic_item("bin123");
        match svc.resolve_payload(&item) {
            PreviewPayload::Image {
                bytes,
                width,
                height,
            } => {
                assert_eq!(*bytes, vec![255u8; 400]);
                assert_eq!(width, 20);
                assert_eq!(height, 20);
            }
            other => panic!("expected Image, got {:?}", other),
        }
    }

    #[test]
    fn dynamic_template_expansion() {
        let exec = MockExec::new().push_ok(true, b"expanded output".to_vec());
        let svc = PreviewService::new(MockCache::new(), exec, MockDecoder::new());
        let item = dynamic_item_with_template("myid", "echo {}");
        assert!(matches!(
            svc.resolve_payload(&item),
            PreviewPayload::Text(ref s) if s == "expanded output"
        ));
    }

    #[test]
    fn picture_nonexistent_returns_value() {
        let svc = PreviewService::new(MockCache::new(), MockExec::new(), MockDecoder::new());
        let item = picture_item("/nonexistent/path/image.png");
        assert!(matches!(
            svc.resolve_payload(&item),
            PreviewPayload::Text(ref s) if s == "/nonexistent/path/image.png"
        ));
    }

    #[test]
    fn picture_cache_hit() {
        let tmp = tempfile::NamedTempFile::new().unwrap();
        let path = tmp.path().to_path_buf();

        let mut cache = MockCache::new();
        let cached_data = vec![128; 100];
        let cache_path = cache.get_cache_path("cat", &path);
        cache = cache.with_valid(cache_path, cached_data.clone(), 5, 5);

        let svc = PreviewService::new(cache, MockExec::new(), MockDecoder::new());
        let item = picture_item(&path.to_string_lossy());
        match svc.resolve_payload(&item) {
            PreviewPayload::Image {
                bytes,
                width,
                height,
            } => {
                assert_eq!(*bytes, cached_data);
                assert_eq!(width, 5);
                assert_eq!(height, 5);
            }
            other => panic!("expected cached Image, got {:?}", other),
        }
    }

    #[test]
    fn picture_cache_miss_generates() {
        let tmp = tempfile::NamedTempFile::new().unwrap();
        let path = tmp.path().to_path_buf();

        let decoder = MockDecoder::new().with_result(vec![64; 80], 4, 5);
        let svc = PreviewService::new(MockCache::new(), MockExec::new(), decoder);
        let item = picture_item(&path.to_string_lossy());
        match svc.resolve_payload(&item) {
            PreviewPayload::Image {
                bytes,
                width,
                height,
            } => {
                assert_eq!(*bytes, vec![64u8; 80]);
                assert_eq!(width, 4);
                assert_eq!(height, 5);
            }
            other => panic!("expected generated Image, got {:?}", other),
        }
    }

    #[test]
    fn picture_video_uses_ffmpeg() {
        let tmp = tempfile::Builder::new().suffix(".mp4").tempfile().unwrap();
        let path = tmp.path().to_path_buf();

        let exec = MockExec::new().push_ok(true, Vec::new());
        let decoder = MockDecoder::new().with_result(vec![200; 160], 8, 10);
        let svc = PreviewService::new(MockCache::new(), exec, decoder);
        let item = picture_item(&path.to_string_lossy());
        match svc.resolve_payload(&item) {
            PreviewPayload::Image {
                bytes,
                width,
                height,
            } => {
                assert_eq!(*bytes, vec![200u8; 160]);
                assert_eq!(width, 8);
                assert_eq!(height, 10);
            }
            other => panic!("expected video thumbnail Image, got {:?}", other),
        }
    }
}
