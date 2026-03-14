use crate::domain::item::Item;
use gdk_pixbuf::Pixbuf;
use gtk4::prelude::*;
use gtk4::{glib, Align, Grid, Label, Picture, ScrolledWindow, TextView};
use std::fs;
use std::path::PathBuf;
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
            .unwrap_or_else(|| std::env::current_dir().unwrap())
            .join("pantry");

        if let Err(e) = fs::create_dir_all(&cache_dir) {
            eprintln!("Warning: Failed to create cache directory: {}", e);
            cache_dir = std::env::current_dir().unwrap_or_else(|_| std::path::PathBuf::from("."));
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
        // 默认显示详细信息区域，文本模式会隐藏它
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

    /// Check if cache should be used (exists and is newer than original)
    fn should_use_cache(&self, cache_path: &std::path::Path, original_path: &std::path::Path) -> bool {
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
    fn load_image_from_cache(&self, cache_path: &std::path::Path, task_id: u64, path_str: &str) {
        let pixbuf_result = Pixbuf::from_file_at_scale(
            cache_path,
            crate::constants::IMAGE_PREVIEW_WIDTH,
            crate::constants::IMAGE_PREVIEW_HEIGHT,
            true,
        );

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

                        // 图片模式：显示详细信息区域
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
                    // 文本内容：隐藏详细信息区域
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

                    let text_view = gtk4::TextView::new();
                    text_view.set_editable(false);
                    text_view.set_cursor_visible(false);
                    text_view.set_wrap_mode(gtk4::WrapMode::Word);
                    text_view.set_left_margin(10);
                    text_view.set_right_margin(10);
                    text_view.set_top_margin(10);
                    text_view.set_bottom_margin(10);

                    let buffer = text_view.buffer();
                    buffer.set_text(&content);

                    text_view.set_hexpand(true);
                    text_view.set_vexpand(true);

                    let scrolled_window = gtk4::ScrolledWindow::new();
                    scrolled_window.set_child(Some(&text_view));
                    scrolled_window.set_hexpand(true);
                    scrolled_window.set_vexpand(true);
                    scrolled_window.set_hscrollbar_policy(gtk4::PolicyType::Automatic);
                    scrolled_window.set_vscrollbar_policy(gtk4::PolicyType::Automatic);

                    Self::set_scrolled_content(&self.content_scrolled, &scrolled_window);
                }
            }
            _ => {
                // 错误情况：显示详细信息区域
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
        // 图片模式：显示详细信息区域
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
            self.load_image_from_cache(&cache_path, task_id, &path_str);
            return;
        }

        let content_scrolled_clone_for_async = self.content_scrolled.clone();
        let current_loading_path_clone = self.current_loading_path.clone();
        let current_task_id_clone = self.current_task_id.clone();
        let cache_path_clone = cache_path.clone();
        let expanded_path_clone = expanded_path.clone();

        self.clear_content();

        let loading_label = Label::new(Some("Loading..."));
        loading_label.set_halign(Align::Center);
        loading_label.set_valign(Align::Center);
        loading_label.set_hexpand(true);
        loading_label.set_vexpand(true);
        Self::set_scrolled_content(&self.content_scrolled, &loading_label);

        glib::spawn_future_local(async move {
            if current_task_id_clone.load(Ordering::SeqCst) != task_id {
                return;
            }

            let result = Pixbuf::from_file_at_scale(
                &expanded_path_clone,
                crate::constants::IMAGE_PREVIEW_WIDTH,
                crate::constants::IMAGE_PREVIEW_HEIGHT,
                true,
            );

            if let Ok(ref pixbuf) = result {
                let format = match expanded_path_clone.extension().and_then(|s| s.to_str()) {
                    Some(ext)
                        if ext.eq_ignore_ascii_case("jpg") || ext.eq_ignore_ascii_case("jpeg") =>
                    {
                        "jpeg"
                    }
                    Some(ext) if ext.eq_ignore_ascii_case("png") => "png",
                    Some(ext) if ext.eq_ignore_ascii_case("bmp") => "bmp",
                    Some(ext)
                        if ext.eq_ignore_ascii_case("tiff") || ext.eq_ignore_ascii_case("tif") =>
                    {
                        "tiff"
                    }
                    Some(ext) if ext.eq_ignore_ascii_case("webp") => "webp",
                    _ => "png",
                };
                let _ = pixbuf.savev(&cache_path_clone, format, &[]);
            }

            let pixbuf_result = result;

            if current_task_id_clone.load(Ordering::SeqCst) != task_id {
                return;
            }

            glib::timeout_add_local(std::time::Duration::from_millis(10), move || {
                if current_task_id_clone.load(Ordering::SeqCst) != task_id {
                    return glib::ControlFlow::Break;
                }

                content_scrolled_clone_for_async.set_child(None::<&gtk4::Widget>);

                if let Ok(ref pixbuf) = pixbuf_result {
                    let picture = Picture::for_pixbuf(pixbuf);
                    picture.set_halign(Align::Center);
                    picture.set_valign(Align::Center);
                    picture.set_hexpand(true);
                    picture.set_vexpand(true);
                    Self::set_scrolled_content(&content_scrolled_clone_for_async, &picture);
                } else {
                    let error_label = Label::new(Some("Failed to load image"));
                    error_label.set_halign(Align::Center);
                    error_label.set_valign(Align::Center);
                    error_label.set_hexpand(true);
                    error_label.set_vexpand(true);
                    Self::set_scrolled_content(&content_scrolled_clone_for_async, &error_label);
                }

                if let Ok(mut current_path) = current_loading_path_clone.lock() {
                    if current_path.as_ref() == Some(&path_str) {
                        *current_path = None;
                    }
                }

                glib::ControlFlow::Break
            });
        });
    }

    fn update_with_text_content(&self, item: &Item) {
        // 文本模式：隐藏详细信息区域（预览区域已显示完整内容）
        self.details_scrolled.set_visible(false);

        self.clear_content();

        let text_view = TextView::new();
        text_view.set_editable(false);
        text_view.set_cursor_visible(false);
        text_view.set_wrap_mode(gtk4::WrapMode::Word);
        text_view.set_left_margin(10);
        text_view.set_right_margin(10);
        text_view.set_top_margin(10);
        text_view.set_bottom_margin(10);

        let buffer = text_view.buffer();
        buffer.set_text(&item.value);

        text_view.set_hexpand(true);
        text_view.set_vexpand(true);

        let scrolled_window = gtk4::ScrolledWindow::new();
        scrolled_window.set_child(Some(&text_view));
        scrolled_window.set_hexpand(true);
        scrolled_window.set_vexpand(true);
        scrolled_window.set_hscrollbar_policy(gtk4::PolicyType::Automatic);
        scrolled_window.set_vscrollbar_policy(gtk4::PolicyType::Automatic);

        Self::set_scrolled_content(&self.content_scrolled, &scrolled_window);
    }
}
