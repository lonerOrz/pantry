use crate::domain::item::Item;
use crate::services::preview::{CacheAdapter, ImageDecoder, PreviewPayload};
use crate::services::process::CommandExecutor;
use std::path::Path;

pub trait PreviewStrategy {
    fn resolve(&self, item: &Item) -> PreviewPayload;
}

pub struct TextPreview;

impl PreviewStrategy for TextPreview {
    fn resolve(&self, item: &Item) -> PreviewPayload {
        PreviewPayload::Text(item.value.clone())
    }
}

pub struct ImagePreview<'a, C: CacheAdapter, E: CommandExecutor, D: ImageDecoder> {
    cache: &'a C,
    executor: &'a E,
    decoder: &'a D,
}

impl<'a, C: CacheAdapter, E: CommandExecutor, D: ImageDecoder> ImagePreview<'a, C, E, D> {
    pub fn new(cache: &'a C, executor: &'a E, decoder: &'a D) -> Self {
        Self {
            cache,
            executor,
            decoder,
        }
    }
}

impl<'a, C: CacheAdapter, E: CommandExecutor, D: ImageDecoder> PreviewStrategy
    for ImagePreview<'a, C, E, D>
{
    fn resolve(&self, item: &Item) -> PreviewPayload {
        let expanded_path = crate::utils::expand_tilde(&item.value);
        if !expanded_path.exists() || !expanded_path.is_file() {
            return PreviewPayload::Text(item.value.clone());
        }

        let cache_path = self.cache.get_cache_path(&item.category, &expanded_path);

        if self.cache.is_cache_valid(&cache_path, &expanded_path)
            && let Some((bytes, w, h)) = self.cache.load_raw_cache(&cache_path)
        {
            return PreviewPayload::Image {
                bytes: std::sync::Arc::new(bytes),
                width: w,
                height: h,
            };
        }

        if crate::services::preview::is_video_file(&expanded_path) {
            generate_video_thumbnail(
                &expanded_path,
                &cache_path,
                self.cache,
                self.decoder,
                self.executor,
            )
        } else {
            generate_image_payload(&expanded_path, &cache_path, self.cache, self.decoder)
        }
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
            bytes: std::sync::Arc::new(bytes),
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
                    bytes: std::sync::Arc::new(raw_data),
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
