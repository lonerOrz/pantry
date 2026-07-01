use crate::cache::CacheAdapter;
use crate::services::process::CommandExecutor;
use std::path::Path;

use super::PreviewPayload;
use super::decoder::ImageDecoder;

pub fn is_video(path: &Path) -> bool {
    match path.extension().and_then(|s| s.to_str()) {
        Some(ext) => {
            let ext_lower = ext.to_lowercase();
            crate::constants::VIDEO_EXTENSIONS.contains(&ext_lower.as_str())
        }
        None => false,
    }
}

pub fn generate_thumbnail(
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
