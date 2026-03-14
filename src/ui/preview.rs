use crate::domain::item::Item;
use gdk_pixbuf::Pixbuf;
use glib::clone;
use gtk4::prelude::*;
use gtk4::{gio, Align, Grid, Label, Picture, ScrolledWindow, TextView};
use image::ImageReader;
use std::fs;
use std::path::PathBuf;
use std::process::Command;
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::{Arc, Mutex};

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
        // Show details area by default, text mode will hide it
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

    /// Create a non-editable text view with standard styling
    fn create_text_view(text: &str) -> TextView {
        let text_view = TextView::new();
        text_view.set_editable(false);
        text_view.set_cursor_visible(false);
        text_view.set_wrap_mode(gtk4::WrapMode::Word);
        text_view.set_left_margin(crate::constants::TEXT_MARGIN);
        text_view.set_right_margin(crate::constants::TEXT_MARGIN);
        text_view.set_top_margin(crate::constants::TEXT_MARGIN);
        text_view.set_bottom_margin(crate::constants::TEXT_MARGIN);

        let buffer = text_view.buffer();
        buffer.set_text(text);

        text_view.set_hexpand(true);
        text_view.set_vexpand(true);
        text_view
    }

    /// Create a scrolled window with standard styling for text content
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
        let thumbnail_path = output_dir.join(format!("{}_thumb.png", video_stem));

        if thumbnail_path.exists() {
            return Some(thumbnail_path);
        }

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
                "-q:v",
                "2",
                &thumbnail_path.to_string_lossy(),
            ])
            .output();

        match result {
            Ok(output) if output.status.success() => Some(thumbnail_path),
            _ => None,
        }
    }

    fn load_image_data(
        path: &std::path::Path,
        max_width: i32,
        max_height: i32,
    ) -> Option<(Vec<u8>, u32, u32)> {
        let img = ImageReader::open(path).ok()?.decode().ok()?;

        let (orig_width, orig_height) = (img.width() as f32, img.height() as f32);
        let (target_width, target_height) = (max_width as f32, max_height as f32);

        let ratio = (target_width / orig_width)
            .min(target_height / orig_height)
            .min(1.0);
        let new_width = (orig_width * ratio) as u32;
        let new_height = (orig_height * ratio) as u32;

        let resized =
            img.resize_exact(new_width, new_height, image::imageops::FilterType::Lanczos3);
        let rgba = resized.to_rgba8();
        let (width, height) = rgba.dimensions();
        let raw_data = rgba.into_raw();

        Some((raw_data, width, height))
    }

    fn create_pixbuf_from_data(raw_data: Vec<u8>, width: u32, height: u32) -> Option<Pixbuf> {
        Some(Pixbuf::from_bytes(
            &glib::Bytes::from(&raw_data),
            gdk_pixbuf::Colorspace::Rgb,
            true,
            8,
            width as i32,
            height as i32,
            (width * 4) as i32,
        ))
    }

    /// Check if cache should be used (exists and is newer than original)
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

    /// Load image from cache and display it
    /// Returns true if successful, false if cache is corrupted (caller should reload from original)
    fn load_image_from_cache(
        &self,
        cache_path: &std::path::Path,
        task_id: u64,
        path_str: &str,
    ) -> bool {
        let pixbuf_result = Pixbuf::from_file_at_scale(
            cache_path,
            crate::constants::IMAGE_PREVIEW_WIDTH,
            crate::constants::IMAGE_PREVIEW_HEIGHT,
            true,
        );

        // If cache is corrupted, delete it and return false (caller will reload from original)
        if pixbuf_result.is_err() {
            let _ = std::fs::remove_file(cache_path);
            return false;
        }

        let content_scrolled_clone = self.content_scrolled.clone();
        let current_loading_path_clone = self.current_loading_path.clone();
        let path_str_clone = path_str.to_string();
        let current_task_id_clone = self.current_task_id.clone();

        glib::idle_add_local(move || {
            if current_task_id_clone.load(Ordering::SeqCst) != task_id {
                return glib::ControlFlow::Break;
            }

            content_scrolled_clone.set_child(None::<&gtk4::Widget>);

            if let Ok(ref pixbuf) = pixbuf_result {
                let picture = Picture::for_pixbuf(pixbuf);
                picture.set_halign(Align::Center);
                picture.set_valign(Align::Center);
                picture.set_hexpand(true);
                picture.set_vexpand(true);
                Self::set_scrolled_content(&content_scrolled_clone, &picture);
            } else {
                let error_label = Label::new(Some("Failed to load image"));
                error_label.set_halign(Align::Center);
                error_label.set_valign(Align::Center);
                error_label.set_hexpand(true);
                error_label.set_vexpand(true);
                Self::set_scrolled_content(&content_scrolled_clone, &error_label);
            }

            if let Ok(mut current_path) = current_loading_path_clone.lock() {
                if current_path.as_ref() == Some(&path_str_clone) {
                    *current_path = None;
                }
            }

            glib::ControlFlow::Break
        });

        true
    }

    fn update_with_dynamic_content(&self, item: &Item) {
        // Use preview_template if available, otherwise default to cliphist decode
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

                        // Image mode: show details area
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
                                // Intentionally leak temp_file to keep it alive for the duration of the preview
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
                    // Text content: hide details area
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
                // Error case: show details area
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
        // Image mode: show details area
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
            "{}_{}",
            item.category,
            crate::utils::path_to_safe_filename(&expanded_path)
        ));

        // Try to load from cache if it exists and is up-to-date
        if self.should_use_cache(&cache_path, &expanded_path) {
            // If cache load succeeds, return early
            // If cache is corrupted, delete it and continue to load from original
            if self.load_image_from_cache(&cache_path, task_id, &path_str) {
                return;
            }
        }

        let cache_path_clone = cache_path.clone();
        let expanded_path_clone = expanded_path.clone();
        let content_scrolled = self.content_scrolled.clone();
        let cache_dir = self.cache_dir.clone();
        let task_id_clone = task_id;
        let current_task_id = self.current_task_id.clone();

        self.clear_content();

        let loading_label = Label::new(Some("Loading..."));
        loading_label.set_halign(Align::Center);
        loading_label.set_valign(Align::Center);
        loading_label.set_hexpand(true);
        loading_label.set_vexpand(true);
        Self::set_scrolled_content(&self.content_scrolled, &loading_label);

        let expanded_path_str = expanded_path_clone.to_string_lossy().to_string();
        let cache_dir_str = cache_dir.to_string_lossy().to_string();
        let cache_path_str = cache_path_clone.to_string_lossy().to_string();

        let expanded_path_str_clone = expanded_path_str.clone();
        let cache_dir_str_clone = cache_dir_str.clone();
        let cache_path_str_clone = cache_path_str.clone();

        glib::spawn_future_local(clone!(
            #[weak]
            content_scrolled,
            async move {
                if current_task_id.load(Ordering::SeqCst) != task_id_clone {
                    return;
                }

                let load_result = gio::spawn_blocking(move || {
                    let expanded_path = std::path::Path::new(&expanded_path_str_clone);
                    let cache_dir_path = std::path::Path::new(&cache_dir_str_clone);

                    let result: Option<(Vec<u8>, u32, u32)>;

                    if Self::is_video_file(expanded_path) {
                        result = Self::generate_video_thumbnail(expanded_path, cache_dir_path)
                            .and_then(|thumb_path| {
                                Self::load_image_data(
                                    &thumb_path,
                                    crate::constants::IMAGE_PREVIEW_WIDTH,
                                    crate::constants::IMAGE_PREVIEW_HEIGHT,
                                )
                            });
                    } else {
                        result = Self::load_image_data(
                            expanded_path,
                            crate::constants::IMAGE_PREVIEW_WIDTH,
                            crate::constants::IMAGE_PREVIEW_HEIGHT,
                        );
                    }

                    result
                }).await;

                if current_task_id.load(Ordering::SeqCst) != task_id_clone {
                    return;
                }

                if let Ok(Some((raw_data, width, height))) = load_result {
                    if let Some(pixbuf) = Self::create_pixbuf_from_data(raw_data, width, height) {
                        let cache_path = std::path::Path::new(&cache_path_str_clone);
                        let _ = pixbuf.savev(cache_path, "png", &[]);
                        let expanded_path = std::path::Path::new(&expanded_path_str);
                        if let Ok(src_meta) = expanded_path.metadata() {
                            if let Ok(src_mtime) = src_meta.modified() {
                                let _ = filetime::set_file_mtime(
                                    cache_path,
                                    filetime::FileTime::from_system_time(src_mtime),
                                );
                            }
                        }

                        content_scrolled.set_child(None::<&gtk4::Widget>);
                        let picture = Picture::for_pixbuf(&pixbuf);
                        picture.set_halign(Align::Center);
                        picture.set_valign(Align::Center);
                        picture.set_hexpand(true);
                        picture.set_vexpand(true);
                        content_scrolled.set_child(Some(&picture));
                    } else {
                        content_scrolled.set_child(None::<&gtk4::Widget>);
                        let error_label = Label::new(Some("Failed to load image"));
                        error_label.set_halign(Align::Center);
                        error_label.set_valign(Align::Center);
                        error_label.set_hexpand(true);
                        error_label.set_vexpand(true);
                        content_scrolled.set_child(Some(&error_label));
                    }
                } else {
                    content_scrolled.set_child(None::<&gtk4::Widget>);
                    let error_label = Label::new(Some("Failed to load image"));
                    error_label.set_halign(Align::Center);
                    error_label.set_valign(Align::Center);
                    error_label.set_hexpand(true);
                    error_label.set_vexpand(true);
                    content_scrolled.set_child(Some(&error_label));
                }
            }
        ));
    }

    fn update_with_text_content(&self, item: &Item) {
        // Text mode: hide details area (full content shown in preview area)
        self.details_scrolled.set_visible(false);

        self.clear_content();

        let text_view = Self::create_text_view(&item.value);
        let scrolled_window = Self::create_text_scrolled(&text_view);

        Self::set_scrolled_content(&self.content_scrolled, &scrolled_window);
    }
}
