use crate::domain::item::Item;
use gdk_pixbuf::Pixbuf;
use glib::clone;
use gtk4::prelude::*;
use gtk4::{Align, Grid, Label, Picture, ScrolledWindow, TextView, gio};
use image::ImageReader;
use std::fs;
use std::path::PathBuf;
use std::process::Command;
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::{Arc, Mutex};

// Timeout for cache loading (in milliseconds)
const CACHE_TIMEOUT_MS: u64 = 300; // 300ms timeout for cache loading

// Timeout for direct image loading (in milliseconds)
const DIRECT_LOAD_TIMEOUT_MS: u64 = 10000; // 10s timeout for direct loading (increased for large images and video thumbnails)

#[derive(Clone)]
pub struct PreviewArea {
    pub container: Grid,
    content_scrolled: ScrolledWindow,
    details_label: Label,
    details_scrolled: ScrolledWindow,
    current_loading_path: Arc<Mutex<Option<String>>>,
    current_task_id: Arc<AtomicU64>,
    next_task_id: Arc<AtomicU64>,
    cache_dir: PathBuf,
}

impl PreviewArea {
    pub fn new() -> Self {
        let mut cache_dir = dirs::cache_dir()
            .unwrap_or_else(|| PathBuf::from("."))
            .join("pantry");

        if let Err(e) = fs::create_dir_all(&cache_dir) {
            eprintln!("Warning: Failed to create cache directory: {}", e);
            cache_dir = PathBuf::from(".");
        }

        let details_label = Label::new(Some("No image selected"));
        details_label.set_wrap(true);
        details_label.set_halign(Align::Start);
        details_label.set_valign(Align::Start);
        details_label.set_ellipsize(gtk4::pango::EllipsizeMode::None);
        details_label.add_css_class("preview-details-label");

        let details_scrolled = gtk4::ScrolledWindow::new();
        details_scrolled.set_child(Some(&details_label));
        details_scrolled.set_vexpand(false);
        details_scrolled.set_hscrollbar_policy(gtk4::PolicyType::Automatic);
        details_scrolled.set_vscrollbar_policy(gtk4::PolicyType::Automatic);
        details_scrolled.set_size_request(-1, 100);
        details_scrolled.add_css_class("preview-details-scrolled");

        let content_scrolled = gtk4::ScrolledWindow::new();
        content_scrolled.set_vexpand(true);
        content_scrolled.set_hexpand(true);
        content_scrolled.set_hscrollbar_policy(gtk4::PolicyType::Automatic);
        content_scrolled.set_vscrollbar_policy(gtk4::PolicyType::Automatic);
        content_scrolled.add_css_class("preview-content-scrolled");

        let separator = gtk4::Separator::new(gtk4::Orientation::Horizontal);
        separator.add_css_class("preview-separator");

        let container = Grid::new();
        container.set_row_homogeneous(false);
        container.set_column_homogeneous(false);
        container.set_vexpand(true);
        container.set_row_spacing(0);

        container.attach(&content_scrolled, 0, 0, 1, 1);
        container.attach(&separator, 0, 1, 1, 1);
        container.attach(&details_scrolled, 0, 2, 1, 1);

        Self {
            container,
            content_scrolled,
            details_label,
            details_scrolled,
            current_loading_path: Arc::new(Mutex::new(None)),
            current_task_id: Arc::new(AtomicU64::new(0)),
            next_task_id: Arc::new(AtomicU64::new(1)),
            cache_dir,
        }
    }

    pub fn update_with_content(&self, item: &Item) {
        self.details_scrolled.set_visible(true);

        match item.source {
            crate::config::SourceMode::Dynamic => self.update_with_dynamic_content(item),
            _ => match item.display {
                crate::config::DisplayMode::Picture => self.update_with_image_content(item),
                crate::config::DisplayMode::Text => self.update_with_text_content(item),
            },
        }
    }

    fn clear_content(&self) {
        self.content_scrolled.set_child(None::<&gtk4::Widget>);
    }

    fn set_scrolled_content<W: IsA<gtk4::Widget>>(scrolled: &ScrolledWindow, widget: &W) {
        scrolled.set_child(Some(widget));
    }

    fn create_text_view(text: &str) -> TextView {
        let text_view = TextView::new();
        text_view.set_editable(false);
        text_view.set_cursor_visible(false);
        text_view.set_wrap_mode(gtk4::WrapMode::Word);
        text_view.set_left_margin(10);
        text_view.set_right_margin(10);
        text_view.set_top_margin(10);
        text_view.set_bottom_margin(10);

        let buffer = text_view.buffer();
        buffer.set_text(text);

        text_view.set_hexpand(true);
        text_view.set_vexpand(true);
        text_view
    }

    fn create_text_scrolled(text_view: &TextView) -> ScrolledWindow {
        let scrolled_window = gtk4::ScrolledWindow::new();
        scrolled_window.set_child(Some(text_view));
        scrolled_window.set_hexpand(true);
        scrolled_window.set_vexpand(true);
        scrolled_window.set_hscrollbar_policy(gtk4::PolicyType::Automatic);
        scrolled_window.set_vscrollbar_policy(gtk4::PolicyType::Automatic);
        scrolled_window
    }

    fn is_video_file(path: &std::path::Path) -> bool {
        match path.extension().and_then(|s| s.to_str()) {
            Some(ext) => matches!(
                ext.to_lowercase().as_str(),
                "mp4" | "webm" | "mkv" | "avi" | "mov" | "wmv" | "flv" | "m4v"
            ),
            None => false,
        }
    }

    fn generate_video_thumbnail(
        video_path: &std::path::Path,
        output_dir: &std::path::Path,
    ) -> Option<PathBuf> {
        let video_stem = video_path.file_stem()?.to_string_lossy();
        // Use .raw extension for consistency
        let thumbnail_path = output_dir.join(format!("{}_thumb.raw", video_stem));

        if thumbnail_path.exists() {
            return Some(thumbnail_path);
        }

        // Generate PNG thumbnail first using ffmpeg (optimized for speed)
        let temp_png = output_dir.join(format!("{}_thumb_temp.png", video_stem));

        let result = Command::new("ffmpeg")
            .args([
                "-y",
                "-ss",
                "1", // Seek to 1 second
                "-i",
                &video_path.to_string_lossy(),
                "-vframes",
                "1",
                "-vf",
                "scale=800:-1",
                "-preset",
                "ultrafast", // Fastest encoding
                "-q:v",
                "5", // Lower quality for speed
                &temp_png.to_string_lossy(),
            ])
            .output();

        match result {
            Ok(output) if output.status.success() => {
                // Convert PNG to RAW format for faster loading
                if let Some((raw_data, width, height)) =
                    Self::load_image_data_raw(&temp_png, 800, 600)
                {
                    let _ = Self::save_raw_cache(&thumbnail_path, &raw_data, width, height);
                    let _ = std::fs::remove_file(temp_png); // Clean up temp PNG
                }
                Some(thumbnail_path)
            }
            Err(e) => {
                eprintln!("[LOAD] ffmpeg error: {}", e);
                let _ = std::fs::remove_file(temp_png);
                None
            }
            _ => {
                let _ = std::fs::remove_file(temp_png);
                None
            }
        }
    }

    /// Load only the first frame of a GIF (faster than loading all frames)
    fn load_gif_first_frame(
        path: &std::path::Path,
        max_width: i32,
        max_height: i32,
    ) -> Option<(Vec<u8>, u32, u32)> {
        // Use ImageReader to load GIF (automatically gets first frame)
        let img = ImageReader::open(path)
            .ok()?
            .with_guessed_format()
            .ok()?
            .decode()
            .ok()?;

        let (width, height) = (img.width(), img.height());

        // Resize if needed
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
        Some((rgba.into_raw(), width, height))
    }

    fn load_image_data_raw(
        path: &std::path::Path,
        max_width: i32,
        max_height: i32,
    ) -> Option<(Vec<u8>, u32, u32)> {
        // Check if this is a GIF - use image crate for first frame only
        let is_gif = path
            .extension()
            .and_then(|s| s.to_str())
            .map(|ext| ext.eq_ignore_ascii_case("gif"))
            .unwrap_or(false);

        if is_gif {
            // For GIF, use image crate to load only first frame (faster)
            return Self::load_gif_first_frame(path, max_width, max_height);
        }

        // Use gdk-pixbuf for fast image loading and scaling (other formats)
        let pixbuf = Pixbuf::from_file_at_scale(
            path, max_width, max_height, true, // preserve aspect ratio
        )
        .ok()?;

        let width = pixbuf.width() as u32;
        let height = pixbuf.height() as u32;
        let has_alpha = pixbuf.has_alpha();
        let rowstride = pixbuf.rowstride() as usize;

        // Get pixel data
        let pixel_bytes = pixbuf.read_pixel_bytes();
        let pixels: &[u8] = pixel_bytes.as_ref();

        // Convert to RGBA format (handle rowstride padding)
        let mut rgba_data = Vec::with_capacity((width * height * 4) as usize);
        let n_channels = pixbuf.n_channels() as usize;

        for y in 0..height as usize {
            let row_start = y * rowstride;
            for x in 0..width as usize {
                let pixel_start = row_start + x * n_channels;
                // Copy RGB channels
                rgba_data.extend_from_slice(&pixels[pixel_start..pixel_start + 3]);
                // Add alpha channel
                if has_alpha && n_channels >= 4 {
                    rgba_data.push(pixels[pixel_start + 3]);
                } else {
                    rgba_data.push(255); // Full alpha
                }
            }
        }

        Some((rgba_data, width, height))
    }

    /// Save raw RGBA data to cache file
    /// Format: [width (4 bytes)][height (4 bytes)][RGBA data]
    fn save_raw_cache(
        path: &std::path::Path,
        raw_data: &[u8],
        width: u32,
        height: u32,
    ) -> std::io::Result<()> {
        use std::io::{BufWriter, Write};
        let file = std::fs::File::create(path)?;
        let mut writer = BufWriter::new(file);
        writer.write_all(&width.to_ne_bytes())?;
        writer.write_all(&height.to_ne_bytes())?;
        writer.write_all(raw_data)?;
        writer.flush()?;
        Ok(())
    }

    /// Load raw RGBA data from cache file
    /// Returns None if file is corrupted
    fn load_raw_cache(path: &std::path::Path) -> Option<(Vec<u8>, u32, u32)> {
        use std::io::Read;

        eprintln!("[CACHE] Loading RAW cache: {:?}", path);
        let start = std::time::Instant::now();

        let mut file = std::fs::File::open(path).ok()?;
        let mut width_buf = [0u8; 4];
        let mut height_buf = [0u8; 4];
        file.read_exact(&mut width_buf).ok()?;
        file.read_exact(&mut height_buf).ok()?;
        let width = u32::from_ne_bytes(width_buf);
        let height = u32::from_ne_bytes(height_buf);

        eprintln!("[CACHE] Dimensions: {}x{}", width, height);

        let expected_size = (width * height * 4) as usize;
        let mut raw_data = Vec::with_capacity(expected_size);
        file.read_to_end(&mut raw_data).ok()?;

        let elapsed = start.elapsed();
        eprintln!(
            "[CACHE] Loaded {} bytes in {:?} (expected: {})",
            raw_data.len(),
            elapsed,
            expected_size
        );

        if raw_data.len() == expected_size {
            Some((raw_data, width, height))
        } else {
            eprintln!("[CACHE] Size mismatch! Corrupted file");
            None // Corrupted
        }
    }

    fn should_use_cache(
        &self,
        cache_path: &std::path::Path,
        original_path: &std::path::Path,
    ) -> bool {
        if !cache_path.exists() {
            return false;
        }

        match (
            original_path.metadata().and_then(|m| m.modified()),
            cache_path.metadata().and_then(|m| m.modified()),
        ) {
            (Ok(original_modified), Ok(cache_metadata)) => cache_metadata >= original_modified,
            _ => false,
        }
    }

    fn load_image_from_cache(&self, cache_path: &std::path::Path, task_id: u64) {
        let cache_path_str = cache_path.to_string_lossy().to_string();
        let content_scrolled = self.content_scrolled.clone();
        let current_task_id = self.current_task_id.clone();
        let cache_path_for_delete = cache_path.to_path_buf();

        let loading_label = Label::new(Some("Loading..."));
        loading_label.set_halign(Align::Center);
        loading_label.set_valign(Align::Center);
        loading_label.set_hexpand(true);
        loading_label.set_vexpand(true);
        Self::set_scrolled_content(&content_scrolled, &loading_label);

        glib::spawn_future_local(clone!(
            #[weak]
            content_scrolled,
            async move {
                if current_task_id.load(Ordering::SeqCst) != task_id {
                    return;
                }

                // Load raw data from cache with timeout
                let load_result = gio::spawn_blocking(move || {
                    eprintln!("[CACHE] Attempting to load: {}", cache_path_str);

                    // Use timeout for cache loading
                    let (tx, rx) = std::sync::mpsc::channel();
                    let cache_path_str_clone = cache_path_str.clone();

                    std::thread::spawn(move || {
                        let result =
                            Self::load_raw_cache(std::path::Path::new(&cache_path_str_clone));
                        let _ = tx.send(result);
                    });

                    // Wait with timeout
                    match rx.recv_timeout(std::time::Duration::from_millis(CACHE_TIMEOUT_MS)) {
                        Ok(result) => {
                            if result.is_none() {
                                eprintln!("[CACHE] Failed to load cache");
                            }
                            result
                        }
                        Err(_) => {
                            eprintln!("[CACHE] Timeout loading cache after {}ms", CACHE_TIMEOUT_MS);
                            None
                        }
                    }
                })
                .await;

                if current_task_id.load(Ordering::SeqCst) != task_id {
                    return;
                }

                content_scrolled.set_child(None::<&gtk4::Widget>);

                // Create Pixbuf on main thread
                if let Ok(Some((raw_data, width, height))) = load_result {
                    let pixbuf = Pixbuf::from_bytes(
                        &glib::Bytes::from(&raw_data),
                        gdk_pixbuf::Colorspace::Rgb,
                        true,
                        8,
                        width as i32,
                        height as i32,
                        (width * 4) as i32,
                    );
                    let picture = Picture::for_pixbuf(&pixbuf);
                    picture.set_halign(Align::Center);
                    picture.set_valign(Align::Center);
                    picture.set_hexpand(true);
                    picture.set_vexpand(true);
                    Self::set_scrolled_content(&content_scrolled, &picture);
                } else {
                    // Cache corrupted or timeout, remove it in background
                    gio::spawn_blocking(move || {
                        let _ = std::fs::remove_file(&cache_path_for_delete);
                    });
                    let error_label = Label::new(Some("Failed to load image"));
                    error_label.set_halign(Align::Center);
                    error_label.set_valign(Align::Center);
                    error_label.set_hexpand(true);
                    error_label.set_vexpand(true);
                    Self::set_scrolled_content(&content_scrolled, &error_label);
                }
            }
        ));
    }

    fn update_with_dynamic_content(&self, item: &Item) {
        let preview_cmd = if let Some(ref template) = item.preview_template {
            template.replace("{}", &item.value)
        } else {
            format!("cliphist decode {}", item.value)
        };

        match std::process::Command::new("sh")
            .arg("-c")
            .arg(&preview_cmd)
            .output()
        {
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
                        Err(_) => {
                            self.details_scrolled.set_visible(true);
                            let details_text = format!(
                                "<b>Title:</b> {}\n<b>Category:</b> {}\n<b>Error:</b> Could not create temporary file",
                                glib::markup_escape_text(&item.title),
                                glib::markup_escape_text(&item.category)
                            );
                            self.details_label.set_markup(&details_text);
                            self.clear_content();
                            let error_label = gtk4::Label::new(Some(
                                "Could not create temporary file for image preview",
                            ));
                            error_label.set_halign(gtk4::Align::Center);
                            error_label.set_valign(gtk4::Align::Center);
                            error_label.set_hexpand(true);
                            error_label.set_vexpand(true);
                            error_label.add_css_class("error-label");
                            Self::set_scrolled_content(&self.content_scrolled, &error_label);
                            return;
                        }
                    };

                    if temp_file.write_all(&output.stdout).is_ok() {
                        let path_str = temp_file.path().to_string_lossy().to_string();

                        self.details_scrolled.set_visible(true);
                        let details_text = format!(
                            "<b>Title:</b> {}\n<b>Category:</b> {}\n<b>Path:</b> {}",
                            glib::markup_escape_text(&item.title),
                            glib::markup_escape_text(&item.category),
                            glib::markup_escape_text(&path_str)
                        );
                        self.details_label.set_markup(&details_text);

                        let adjustment = self.details_scrolled.vadjustment();
                        adjustment.set_value(0.0);
                        self.clear_content();

                        match gdk_pixbuf::Pixbuf::from_file_at_scale(
                            temp_file.path(),
                            crate::constants::IMAGE_PREVIEW_WIDTH,
                            crate::constants::IMAGE_PREVIEW_HEIGHT,
                            true,
                        ) {
                            Ok(texture) => {
                                let picture = gtk4::Picture::for_pixbuf(&texture);
                                picture.set_halign(gtk4::Align::Center);
                                picture.set_valign(gtk4::Align::Center);
                                picture.set_hexpand(true);
                                picture.set_vexpand(true);
                                picture.add_css_class("preview-image");
                                Self::set_scrolled_content(&self.content_scrolled, &picture);
                                std::mem::forget(temp_file);
                            }
                            Err(_) => {
                                let error_label = gtk4::Label::new(Some(&format!(
                                    "Failed to load image from temp file:\n{}",
                                    path_str
                                )));
                                error_label.set_halign(gtk4::Align::Center);
                                error_label.set_valign(gtk4::Align::Center);
                                error_label.set_hexpand(true);
                                error_label.set_vexpand(true);
                                error_label.add_css_class("error-label");
                                Self::set_scrolled_content(&self.content_scrolled, &error_label);
                            }
                        }
                    } else {
                        let error_label =
                            gtk4::Label::new(Some("Failed to write image data to temporary file"));
                        error_label.set_halign(gtk4::Align::Center);
                        error_label.set_valign(gtk4::Align::Center);
                        error_label.set_hexpand(true);
                        error_label.set_vexpand(true);
                        error_label.add_css_class("error-label");
                        Self::set_scrolled_content(&self.content_scrolled, &error_label);
                    }
                } else {
                    self.details_scrolled.set_visible(false);

                    let details_text = format!(
                        "<b>Title:</b> {}\n<b>Category:</b> {}",
                        glib::markup_escape_text(&item.title),
                        glib::markup_escape_text(&item.category)
                    );
                    self.details_label.set_markup(&details_text);

                    let adjustment = self.details_scrolled.vadjustment();
                    adjustment.set_value(0.0);
                    self.clear_content();

                    let content = String::from_utf8_lossy(&output.stdout);

                    let text_view = Self::create_text_view(&content);
                    let scrolled_window = Self::create_text_scrolled(&text_view);

                    Self::set_scrolled_content(&self.content_scrolled, &scrolled_window);
                }
            }
            _ => {
                self.details_scrolled.set_visible(true);

                let details_text = format!(
                    "<b>Title:</b> {}\n<b>Category:</b> {}\n<b>Error:</b> Failed to execute preview command",
                    glib::markup_escape_text(&item.title),
                    glib::markup_escape_text(&item.category)
                );
                self.details_label.set_markup(&details_text);
                self.clear_content();

                let error_label = gtk4::Label::new(Some("Failed to execute preview command"));
                error_label.set_halign(gtk4::Align::Center);
                error_label.set_valign(gtk4::Align::Center);
                error_label.set_hexpand(true);
                error_label.set_vexpand(true);
                error_label.add_css_class("error-label");
                Self::set_scrolled_content(&self.content_scrolled, &error_label);
            }
        }
    }

    fn update_with_image_content(&self, item: &Item) {
        self.details_scrolled.set_visible(true);

        let path = std::path::Path::new(&item.value);
        if !path.exists() || !path.is_file() {
            self.update_with_text_content(item);
            return;
        }

        let image_path = &item.value;
        let expanded_path = crate::utils::expand_tilde(image_path);
        let path_str = expanded_path.to_string_lossy().to_string();

        let details_text = format!(
            "<b>Title:</b> {}\n<b>Category:</b> {}\n<b>Path:</b> {}",
            glib::markup_escape_text(&item.title),
            glib::markup_escape_text(&item.category),
            glib::markup_escape_text(&item.value)
        );
        self.details_label.set_markup(&details_text);

        let adjustment = self.details_scrolled.vadjustment();
        adjustment.set_value(0.0);

        if let Ok(current_path) = self.current_loading_path.lock() {
            if current_path.as_ref() == Some(&path_str) {
                return;
            }
        }

        let task_id = self.next_task_id.fetch_add(1, Ordering::SeqCst);

        if let Ok(mut current_path) = self.current_loading_path.lock() {
            *current_path = Some(path_str.clone());
        }

        self.current_task_id.store(task_id, Ordering::SeqCst);

        if !expanded_path.exists() {
            glib::idle_add_local({
                let content_scrolled_clone = self.content_scrolled.clone();
                let path_str_clone = path_str.clone();

                move || {
                    content_scrolled_clone.set_child(None::<&gtk4::Widget>);
                    let error_label =
                        Label::new(Some(&format!("File does not exist:\n{}", path_str_clone)));
                    error_label.set_halign(Align::Center);
                    error_label.set_valign(Align::Center);
                    error_label.set_hexpand(true);
                    error_label.set_vexpand(true);
                    Self::set_scrolled_content(&content_scrolled_clone, &error_label);
                    glib::ControlFlow::Break
                }
            });
            return;
        }

        let cache_path = self.cache_dir.join(format!(
            "{}_{}.raw",
            item.category,
            crate::utils::path_to_safe_filename(&expanded_path)
        ));

        // Check if this is a video that needs thumbnail generation
        // Note: GIF can be loaded directly, only video needs ffmpeg
        let needs_thumbnail = Self::is_video_file(&expanded_path);

        // Try to load from cache if it exists and is up-to-date
        if self.should_use_cache(&cache_path, &expanded_path) {
            self.load_image_from_cache(&cache_path, task_id);
            return;
        }

        // No cache available
        if needs_thumbnail {
            // For video: must generate thumbnail first, then display
            eprintln!(
                "[LOAD] No cache, generating thumbnail for: {}",
                expanded_path.display()
            );
            self.generate_and_display_thumbnail(&expanded_path, &cache_path, task_id);
        } else {
            // For static images and GIF: load directly and save cache in background
            eprintln!(
                "[LOAD] No cache, loading original for: {}",
                expanded_path.display()
            );
            self.load_original_with_background_cache(&expanded_path, &cache_path, task_id);
        }
    }

    /// Generate thumbnail and display (for video)
    fn generate_and_display_thumbnail(
        &self,
        expanded_path: &std::path::Path,
        cache_path: &std::path::Path,
        task_id: u64,
    ) {
        let expanded_path_clone = expanded_path.to_path_buf();
        let _cache_path_clone = cache_path.to_path_buf();
        let cache_dir = self.cache_dir.clone();
        let content_scrolled = self.content_scrolled.clone();
        let current_task_id = self.current_task_id.clone();
        let expected_task_id = task_id;

        self.clear_content();
        let loading_label = Label::new(Some("Generating thumbnail..."));
        loading_label.set_halign(Align::Center);
        loading_label.set_valign(Align::Center);
        loading_label.set_hexpand(true);
        loading_label.set_vexpand(true);
        Self::set_scrolled_content(&content_scrolled, &loading_label);

        glib::spawn_future_local(clone!(
            #[weak]
            content_scrolled,
            async move {
                if current_task_id.load(Ordering::SeqCst) != expected_task_id {
                    return;
                }

                // Generate thumbnail (this may take time for videos)
                let result = gio::spawn_blocking(move || {
                    if Self::is_video_file(&expanded_path_clone) {
                        // Generate video thumbnail - this saves to cache_path_clone
                        match Self::generate_video_thumbnail(&expanded_path_clone, &cache_dir) {
                            Some(thumb_path) => {
                                eprintln!("[LOAD] Video thumbnail generated: {:?}", thumb_path);
                                // Load from the generated RAW cache
                                Self::load_raw_cache(&thumb_path)
                            }
                            None => {
                                eprintln!("[LOAD] Video thumbnail generation failed");
                                None
                            }
                        }
                    } else {
                        // GIF - load directly
                        Self::load_image_data_raw(
                            &expanded_path_clone,
                            crate::constants::IMAGE_PREVIEW_WIDTH,
                            crate::constants::IMAGE_PREVIEW_HEIGHT,
                        )
                    }
                })
                .await;

                if current_task_id.load(Ordering::SeqCst) != expected_task_id {
                    return;
                }

                content_scrolled.set_child(None::<&gtk4::Widget>);

                if let Ok(Some((raw_data, width, height))) = result {
                    // Display the loaded thumbnail
                    let pixbuf = Pixbuf::from_bytes(
                        &glib::Bytes::from(&raw_data),
                        gdk_pixbuf::Colorspace::Rgb,
                        true,
                        8,
                        width as i32,
                        height as i32,
                        (width * 4) as i32,
                    );
                    let picture = Picture::for_pixbuf(&pixbuf);
                    picture.set_halign(Align::Center);
                    picture.set_valign(Align::Center);
                    picture.set_hexpand(true);
                    picture.set_vexpand(true);
                    Self::set_scrolled_content(&content_scrolled, &picture);
                    eprintln!("[LOAD] Thumbnail displayed");
                } else {
                    eprintln!("[LOAD] Failed to display thumbnail");
                    let error_label = Label::new(Some("Failed to generate thumbnail"));
                    error_label.set_halign(Align::Center);
                    error_label.set_valign(Align::Center);
                    error_label.set_hexpand(true);
                    error_label.set_vexpand(true);
                    error_label.add_css_class("error-label");
                    Self::set_scrolled_content(&content_scrolled, &error_label);
                }
            }
        ));
    }

    /// Load original image and save cache in background (for static images)
    fn load_original_with_background_cache(
        &self,
        expanded_path: &std::path::Path,
        cache_path: &std::path::Path,
        task_id: u64,
    ) {
        let expanded_path_clone = expanded_path.to_path_buf();
        let cache_path_clone = cache_path.to_path_buf();
        let content_scrolled = self.content_scrolled.clone();
        let current_task_id = self.current_task_id.clone();
        let expected_task_id = task_id;

        self.clear_content();
        let loading_label = Label::new(Some("Loading..."));
        loading_label.set_halign(Align::Center);
        loading_label.set_valign(Align::Center);
        loading_label.set_hexpand(true);
        loading_label.set_vexpand(true);
        Self::set_scrolled_content(&content_scrolled, &loading_label);

        glib::spawn_future_local(clone!(
            #[weak]
            content_scrolled,
            async move {
                if current_task_id.load(Ordering::SeqCst) != expected_task_id {
                    return;
                }

                // Load original with timeout
                let load_result = gio::spawn_blocking(move || {
                    let (tx, rx) = std::sync::mpsc::channel();
                    let expanded_path_clone_copy = expanded_path_clone.clone();

                    std::thread::spawn(move || {
                        let result = Self::load_image_data_raw(
                            &expanded_path_clone_copy,
                            crate::constants::IMAGE_PREVIEW_WIDTH,
                            crate::constants::IMAGE_PREVIEW_HEIGHT,
                        );
                        let _ = tx.send(result);
                    });

                    match rx.recv_timeout(std::time::Duration::from_millis(DIRECT_LOAD_TIMEOUT_MS))
                    {
                        Ok(r) => r,
                        Err(_) => {
                            eprintln!("[LOAD] Timeout after {}ms", DIRECT_LOAD_TIMEOUT_MS);
                            None
                        }
                    }
                })
                .await;

                if current_task_id.load(Ordering::SeqCst) != expected_task_id {
                    return;
                }

                content_scrolled.set_child(None::<&gtk4::Widget>);

                if let Ok(Some((raw_data, width, height))) = load_result {
                    // Save cache in background (don't wait)
                    let cache_path_bg = cache_path_clone.clone();
                    let raw_data_bg = raw_data.clone();
                    glib::spawn_future_local(async move {
                        let _ = gio::spawn_blocking(move || {
                            let _ =
                                Self::save_raw_cache(&cache_path_bg, &raw_data_bg, width, height);
                        })
                        .await;
                    });

                    let pixbuf = Pixbuf::from_bytes(
                        &glib::Bytes::from(&raw_data),
                        gdk_pixbuf::Colorspace::Rgb,
                        true,
                        8,
                        width as i32,
                        height as i32,
                        (width * 4) as i32,
                    );
                    let picture = Picture::for_pixbuf(&pixbuf);
                    picture.set_halign(Align::Center);
                    picture.set_valign(Align::Center);
                    picture.set_hexpand(true);
                    picture.set_vexpand(true);
                    Self::set_scrolled_content(&content_scrolled, &picture);
                } else {
                    let error_label = Label::new(Some("Failed to load image"));
                    error_label.set_halign(Align::Center);
                    error_label.set_valign(Align::Center);
                    error_label.set_hexpand(true);
                    error_label.set_vexpand(true);
                    error_label.add_css_class("error-label");
                    Self::set_scrolled_content(&content_scrolled, &error_label);
                }
            }
        ));
    }

    fn update_with_text_content(&self, item: &Item) {
        self.details_scrolled.set_visible(false);

        self.clear_content();

        let text_view = Self::create_text_view(&item.value);
        let scrolled_window = Self::create_text_scrolled(&text_view);

        Self::set_scrolled_content(&self.content_scrolled, &scrolled_window);
    }
}
