use crate::cache::CacheManager;
use crate::domain::item::Item;
use gdk_pixbuf::Pixbuf;
use image::ImageReader;
use std::path::Path;
use std::process::Command;
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

pub struct PreviewService {
    cache: CacheManager,
}

impl PreviewService {
    pub fn new() -> Self {
        Self {
            cache: CacheManager::new(),
        }
    }

    pub fn resolve_payload(&self, item: &Item) -> PreviewPayload {
        if item.preview_template.is_some()
            || matches!(item.source, crate::config::SourceMode::Dynamic)
        {
            return self.resolve_dynamic(item);
        }

        match item.display {
            crate::config::DisplayMode::Picture => self.resolve_picture(item),
            crate::config::DisplayMode::Text => PreviewPayload::Text(item.value.clone()),
        }
    }

    fn resolve_picture(&self, item: &Item) -> PreviewPayload {
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
            self.generate_video_thumbnail(&expanded_path, &cache_path)
        } else {
            self.generate_image_payload(&expanded_path, &cache_path)
        }
    }

    fn generate_image_payload(&self, path: &Path, cache_path: &Path) -> PreviewPayload {
        let max_w = crate::constants::IMAGE_PREVIEW_WIDTH;
        let max_h = crate::constants::IMAGE_PREVIEW_HEIGHT;

        let result = if path
            .extension()
            .and_then(|s| s.to_str())
            .is_some_and(|ext| ext.eq_ignore_ascii_case("gif"))
        {
            load_gif_first_frame(path, max_w, max_h)
        } else {
            load_image_data_raw(path, max_w, max_h)
        };

        if let Some((bytes, w, h)) = result {
            let _ = self.cache.save_raw_cache(cache_path, &bytes, w, h);
            PreviewPayload::Image {
                bytes: Arc::new(bytes),
                width: w,
                height: h,
            }
        } else {
            PreviewPayload::Error("Failed to decode image".to_string())
        }
    }

    fn generate_video_thumbnail(&self, video_path: &Path, cache_path: &Path) -> PreviewPayload {
        let video_stem = match video_path.file_stem().and_then(|s| s.to_str()) {
            Some(s) => s,
            None => return PreviewPayload::Error("Invalid video filename".to_string()),
        };

        let temp_png = cache_path.with_file_name(format!("{}_thumb_temp.png", video_stem));

        let result = Command::new("ffmpeg")
            .args([
                "-y",
                "-ss",
                "1",
                "-i",
                &video_path.to_string_lossy(),
                "-vframes",
                "1",
                "-vf",
                "scale=800:-1",
                "-preset",
                "ultrafast",
                "-q:v",
                "5",
                &temp_png.to_string_lossy(),
            ])
            .output();

        match result {
            Ok(output) if output.status.success() => {
                if let Some((raw_data, w, h)) = load_image_data_raw(&temp_png, 800, 600) {
                    let _ = self.cache.save_raw_cache(cache_path, &raw_data, w, h);
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

    fn resolve_dynamic(&self, item: &Item) -> PreviewPayload {
        let preview_cmd = if let Some(ref template) = item.preview_template {
            template.replace("{}", &item.value)
        } else {
            format!("cliphist decode {}", item.value)
        };

        match Command::new("sh").arg("-c").arg(&preview_cmd).output() {
            Ok(output) if output.status.success() => {
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
                            load_image_data_raw(temp_file.path(), max_w, max_h)
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

fn load_gif_first_frame(
    path: &Path,
    max_width: i32,
    max_height: i32,
) -> Option<(Vec<u8>, i32, i32)> {
    let img = ImageReader::open(path)
        .ok()?
        .with_guessed_format()
        .ok()?
        .decode()
        .ok()?;

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

    let pixbuf = Pixbuf::from_file_at_scale(path, max_width, max_height, true).ok()?;
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
