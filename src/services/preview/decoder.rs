use gdk_pixbuf::Pixbuf;
use image::ImageReader;
use std::path::Path;

pub trait ImageDecoder: Send + Sync {
    fn load_from_path(
        &self,
        path: &Path,
        max_width: i32,
        max_height: i32,
    ) -> Option<(Vec<u8>, i32, i32)>;
}

#[derive(Clone)]
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

fn load_gif_first_frame(
    path: &Path,
    max_width: i32,
    max_height: i32,
) -> Option<(Vec<u8>, i32, i32)> {
    let reader = ImageReader::open(path).ok()?.with_guessed_format().ok()?;
    let (orig_w, orig_h) = reader.into_dimensions().ok()?;
    let pixel_bytes = orig_w as u64 * orig_h as u64 * 4;

    if pixel_bytes > crate::constants::MAX_DECODE_PIXEL_BYTES {
        return None;
    }

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
