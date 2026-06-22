use crate::cache::CacheManager;
use crate::domain::item::Item;
use gdk_pixbuf::Pixbuf;
use image::ImageReader;
use std::io;
use std::path::{Path, PathBuf};
use std::sync::Arc;

#[derive(Debug, Clone)]
pub enum PreviewPayload {
    #[allow(dead_code)]
    None,
    Text(String),
    Image {
        bytes: Arc<Vec<u8>>,
        width: i32,
        height: i32,
    },
    Error(String),
}

pub use crate::services::process::{CommandExecutor, ShellExec};

pub trait CacheAdapter: Send + Sync {
    fn get_cache_path(&self, category: &str, original_path: &Path) -> PathBuf;
    fn is_cache_valid(&self, cache_path: &Path, original_path: &Path) -> bool;
    fn save_raw_cache(
        &self,
        path: &Path,
        raw_data: &[u8],
        width: i32,
        height: i32,
    ) -> io::Result<()>;
    fn load_raw_cache(&self, path: &Path) -> Option<(Vec<u8>, i32, i32)>;
}

pub trait ImageDecoder: Send + Sync {
    fn load_from_path(
        &self,
        path: &Path,
        max_width: i32,
        max_height: i32,
    ) -> Option<(Vec<u8>, i32, i32)>;
}

impl CacheAdapter for CacheManager {
    fn get_cache_path(&self, category: &str, original_path: &Path) -> PathBuf {
        self.get_cache_path(category, original_path)
    }

    fn is_cache_valid(&self, cache_path: &Path, original_path: &Path) -> bool {
        self.is_cache_valid(cache_path, original_path)
    }

    fn save_raw_cache(
        &self,
        path: &Path,
        raw_data: &[u8],
        width: i32,
        height: i32,
    ) -> io::Result<()> {
        self.save_raw_cache(path, raw_data, width, height)
    }

    fn load_raw_cache(&self, path: &Path) -> Option<(Vec<u8>, i32, i32)> {
        self.load_raw_cache(path)
    }
}

pub struct GdkPixbufDecoder;

impl ImageDecoder for GdkPixbufDecoder {
    fn load_from_path(
        &self,
        path: &Path,
        max_width: i32,
        max_height: i32,
    ) -> Option<(Vec<u8>, i32, i32)> {
        if path
            .extension()
            .and_then(|s| s.to_str())
            .is_some_and(|ext| ext.eq_ignore_ascii_case("gif"))
        {
            load_gif_first_frame(path, max_width, max_height)
        } else {
            load_image_data_raw(path, max_width, max_height)
        }
    }
}

pub type ProdPreviewService = PreviewService<CacheManager, ShellExec, GdkPixbufDecoder>;

pub fn create_prod_preview_service() -> ProdPreviewService {
    ProdPreviewService::new(
        crate::cache::CacheManager::new(),
        ShellExec,
        GdkPixbufDecoder,
    )
}

pub struct PreviewService<C: CacheAdapter, E: CommandExecutor, D: ImageDecoder> {
    cache: C,
    executor: E,
    decoder: D,
}

impl<C: CacheAdapter, E: CommandExecutor, D: ImageDecoder> PreviewService<C, E, D> {
    pub fn new(cache: C, executor: E, decoder: D) -> Self {
        Self {
            cache,
            executor,
            decoder,
        }
    }

    pub fn try_cache(&self, item: &Item) -> Option<PreviewPayload> {
        if item.preview_template.is_some()
            || matches!(item.source, crate::config::SourceMode::Dynamic)
        {
            return None;
        }
        if !matches!(item.display, crate::config::DisplayMode::Picture) {
            return None;
        }
        let expanded_path = crate::utils::expand_tilde(&item.value);
        if !expanded_path.exists() || !expanded_path.is_file() {
            return None;
        }
        if is_video_file(&expanded_path) {
            return None;
        }
        let cache_path = self.cache.get_cache_path(&item.category, &expanded_path);
        if self.cache.is_cache_valid(&cache_path, &expanded_path)
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

    pub fn resolve_payload(&self, item: &Item) -> PreviewPayload {
        if item.preview_template.is_some()
            || matches!(item.source, crate::config::SourceMode::Dynamic)
        {
            return self.resolve_dynamic(item);
        }

        match item.display {
            crate::config::DisplayMode::Text => PreviewPayload::Text(item.value.clone()),
            crate::config::DisplayMode::Picture => self.resolve_image(item),
        }
    }

    fn resolve_image(&self, item: &Item) -> PreviewPayload {
        let expanded_path = crate::utils::expand_tilde(&item.value);
        if !expanded_path.exists() || !expanded_path.is_file() {
            return PreviewPayload::Text(item.value.clone());
        }

        let cache_path = self.cache.get_cache_path(&item.category, &expanded_path);

        if self.cache.is_cache_valid(&cache_path, &expanded_path)
            && let Some((bytes, w, h)) = self.cache.load_raw_cache(&cache_path)
        {
            return PreviewPayload::Image {
                bytes: Arc::new(bytes),
                width: w,
                height: h,
            };
        }

        if is_video_file(&expanded_path) {
            generate_video_thumbnail(
                &expanded_path,
                &cache_path,
                &self.cache,
                &self.decoder,
                &self.executor,
            )
        } else {
            generate_image_payload(&expanded_path, &cache_path, &self.cache, &self.decoder)
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
                let is_binary = output
                    .stdout
                    .iter()
                    .any(|&b| b == 0 || (b < 32 && b != b'\n' && b != b'\t'));

                if is_binary {
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

fn is_video_file(path: &Path) -> bool {
    match path.extension().and_then(|s| s.to_str()) {
        Some(ext) => matches!(
            ext.to_lowercase().as_str(),
            "mp4" | "webm" | "mkv" | "avi" | "mov" | "wmv" | "flv" | "m4v"
        ),
        None => false,
    }
}

fn generate_image_payload(
    path: &Path,
    cache_path: &Path,
    cache: &dyn CacheAdapter,
    decoder: &dyn ImageDecoder,
) -> PreviewPayload {
    let max_w = crate::constants::IMAGE_PREVIEW_WIDTH;
    let max_h = crate::constants::IMAGE_PREVIEW_HEIGHT;

    if let Some((bytes, w, h)) = decoder.load_from_path(path, max_w, max_h) {
        let _ = cache.save_raw_cache(cache_path, &bytes, w, h);
        PreviewPayload::Image {
            bytes: Arc::new(bytes),
            width: w,
            height: h,
        }
    } else {
        PreviewPayload::Error("Failed to decode image".to_string())
    }
}

fn generate_video_thumbnail(
    video_path: &Path,
    cache_path: &Path,
    cache: &dyn CacheAdapter,
    decoder: &dyn ImageDecoder,
    executor: &dyn CommandExecutor,
) -> PreviewPayload {
    let video_stem = match video_path.file_stem().and_then(|s| s.to_str()) {
        Some(s) => s,
        None => return PreviewPayload::Error("Invalid video filename".to_string()),
    };

    let temp_png = cache_path.with_file_name(format!("{}_thumb_temp.png", video_stem));

    let video_str = video_path.to_string_lossy();
    let temp_str = temp_png.to_string_lossy();
    let scale = crate::constants::FFMPEG_THUMB_SCALE;
    let quality = crate::constants::FFMPEG_THUMB_QUALITY;
    let args: [&str; 14] = [
        "-y",
        "-ss",
        "1",
        "-i",
        &video_str,
        "-vframes",
        "1",
        "-vf",
        &format!("scale={scale}:-1"),
        "-preset",
        "ultrafast",
        "-q:v",
        &quality.to_string(),
        &temp_str,
    ];

    match executor.execute("ffmpeg", &args) {
        Ok(output) if output.success => {
            if let Some((raw_data, w, h)) =
                decoder.load_from_path(&temp_png, scale, crate::constants::IMAGE_PREVIEW_HEIGHT)
            {
                let _ = cache.save_raw_cache(cache_path, &raw_data, w, h);
                let _ = std::fs::remove_file(&temp_png);
                PreviewPayload::Image {
                    bytes: Arc::new(raw_data),
                    width: w,
                    height: h,
                }
            } else {
                let _ = std::fs::remove_file(&temp_png);
                PreviewPayload::Error("Failed to decode video thumbnail".to_string())
            }
        }
        _ => {
            let _ = std::fs::remove_file(&temp_png);
            PreviewPayload::Error("FFmpeg execution failed".to_string())
        }
    }
}

fn load_gif_first_frame(
    path: &Path,
    max_width: i32,
    max_height: i32,
) -> Option<(Vec<u8>, i32, i32)> {
    let reader = match ImageReader::open(path) {
        Ok(r) => r,
        Err(e) => {
            eprintln!("Failed to open image file {}: {}", path.display(), e);
            return None;
        }
    };
    let reader = match reader.with_guessed_format() {
        Ok(r) => r,
        Err(e) => {
            eprintln!("Failed to guess format for {}: {}", path.display(), e);
            return None;
        }
    };

    let (orig_w, orig_h) = match reader.into_dimensions() {
        Ok(d) => d,
        Err(e) => {
            eprintln!("Failed to get dimensions for {}: {}", path.display(), e);
            return None;
        }
    };
    let pixel_bytes = orig_w as u64 * orig_h as u64 * 4;
    if pixel_bytes > crate::constants::MAX_DECODE_PIXEL_BYTES {
        return None;
    }

    let img = match ImageReader::open(path) {
        Ok(r) => r,
        Err(e) => {
            eprintln!("Failed to open image {}: {}", path.display(), e);
            return None;
        }
    };

    let reader = match img.with_guessed_format() {
        Ok(r) => r,
        Err(e) => {
            eprintln!("Failed to guess format for {}: {}", path.display(), e);
            return None;
        }
    };

    let img = match reader.decode() {
        Ok(i) => i,
        Err(e) => {
            eprintln!("Failed to decode image {}: {}", path.display(), e);
            return None;
        }
    };

    let (width, height) = (img.width(), img.height());

    let (target_width, target_height) = {
        let ratio = (max_width as f32 / width as f32)
            .min(max_height as f32 / height as f32)
            .min(1.0);
        (
            (width as f32 * ratio) as u32,
            (height as f32 * ratio) as u32,
        )
    };

    let resized = if target_width < width || target_height < height {
        img.resize_exact(
            target_width,
            target_height,
            image::imageops::FilterType::Lanczos3,
        )
    } else {
        img
    };

    let rgba = resized.to_rgba8();
    let (width, height) = (rgba.width(), rgba.height());
    Some((rgba.into_raw(), width as i32, height as i32))
}

fn load_image_data_raw(
    path: &Path,
    max_width: i32,
    max_height: i32,
) -> Option<(Vec<u8>, i32, i32)> {
    if path
        .extension()
        .and_then(|s| s.to_str())
        .is_some_and(|ext| ext.eq_ignore_ascii_case("gif"))
    {
        return load_gif_first_frame(path, max_width, max_height);
    }

    let pixbuf = match Pixbuf::from_file_at_scale(path, max_width, max_height, true) {
        Ok(pb) => pb,
        Err(e) => {
            eprintln!("Failed to load Pixbuf from file {}: {}", path.display(), e);
            return None;
        }
    };
    let width = pixbuf.width();
    let height = pixbuf.height();
    let has_alpha = pixbuf.has_alpha();
    let rowstride = pixbuf.rowstride() as usize;

    let pixel_bytes = pixbuf.read_pixel_bytes();
    let pixels: &[u8] = pixel_bytes.as_ref();

    let mut rgba_data = Vec::with_capacity((width * height * 4) as usize);
    let n_channels = pixbuf.n_channels() as usize;

    for y in 0..height as usize {
        let row_start = y * rowstride;
        for x in 0..width as usize {
            let pixel_start = row_start + x * n_channels;
            if pixel_start + n_channels > pixels.len() {
                break;
            }

            match n_channels {
                1 => {
                    let v = pixels[pixel_start];
                    rgba_data.extend_from_slice(&[v, v, v, 255]);
                }
                2 => {
                    let v = pixels[pixel_start];
                    let a = pixels[pixel_start + 1];
                    rgba_data.extend_from_slice(&[v, v, v, a]);
                }
                _ => {
                    rgba_data.extend_from_slice(&pixels[pixel_start..pixel_start + 3]);
                    let a = if has_alpha && n_channels >= 4 {
                        pixels[pixel_start + 3]
                    } else {
                        255
                    };
                    rgba_data.push(a);
                }
            }
        }
    }

    Some((rgba_data, width, height))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::domain::{DisplayMode, SourceMode};
    use crate::services::process::MockExec;
    use std::collections::HashMap;
    use std::sync::RwLock;

    type CacheEntry = (Vec<u8>, i32, i32);

    struct MockCache {
        valid_entries: HashMap<PathBuf, bool>,
        stored: RwLock<HashMap<PathBuf, CacheEntry>>,
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
        Item::builder()
            .title("test".into())
            .value(value.into())
            .category("cat".into())
            .display(DisplayMode::Text)
            .source(SourceMode::Config)
            .build()
    }

    fn dynamic_item(value: &str) -> Item {
        Item::builder()
            .title("test".into())
            .value(value.into())
            .category("cat".into())
            .display(DisplayMode::Text)
            .source(SourceMode::Dynamic)
            .build()
    }

    fn dynamic_item_with_template(value: &str, template: &str) -> Item {
        Item::builder()
            .title("test".into())
            .value(value.into())
            .category("cat".into())
            .display(DisplayMode::Text)
            .source(SourceMode::Dynamic)
            .preview_template(template.into())
            .build()
    }

    fn picture_item(path: &str) -> Item {
        Item::builder()
            .title("test".into())
            .value(path.into())
            .category("cat".into())
            .display(DisplayMode::Picture)
            .source(SourceMode::Config)
            .build()
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
