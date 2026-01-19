use crate::domain::item::Item;
use gdk_pixbuf::Pixbuf;
use gtk4::prelude::*;
use gtk4::{
    glib, Align, Box as GtkBox, Grid, Label, Orientation, Picture, ScrolledWindow, TextView,
};
use std::fs;
use std::path::PathBuf;
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::{Arc, Mutex};

#[derive(Clone)]
pub struct PreviewArea {
    pub container: Grid,
    image_container: GtkBox,
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
            // Use fallback directory
            cache_dir = std::env::current_dir().unwrap_or_else(|_| std::path::PathBuf::from("."));
        }

        let image_container = GtkBox::new(Orientation::Vertical, 0);
        image_container.set_vexpand(true);
        image_container.set_hexpand(true);
        image_container.set_homogeneous(false);

        let details_label = Label::new(Some("No image selected"));
        details_label.set_wrap(false);
        details_label.set_halign(Align::Start);
        details_label.set_ellipsize(gtk4::pango::EllipsizeMode::None);
        details_label.add_css_class("preview-details-label");

        let details_scrolled = gtk4::ScrolledWindow::new();
        details_scrolled.set_child(Some(&details_label));
        details_scrolled.set_vexpand(false);
        details_scrolled.set_hscrollbar_policy(gtk4::PolicyType::Automatic);
        details_scrolled.set_vscrollbar_policy(gtk4::PolicyType::Automatic);
        details_scrolled.set_size_request(-1, 80);
        details_scrolled.add_css_class("preview-details-scrolled");

        details_scrolled.set_propagate_natural_height(false);
        details_scrolled.set_propagate_natural_width(false);

        let separator = gtk4::Separator::new(gtk4::Orientation::Horizontal);
        separator.add_css_class("preview-separator");

        let container = Grid::new();
        container.set_row_homogeneous(false);
        container.set_column_homogeneous(false);
        container.set_vexpand(true);
        container.set_row_spacing(0);

        container.attach(&image_container, 0, 0, 1, 1);
        container.attach(&separator, 0, 1, 1, 1);
        container.attach(&details_scrolled, 0, 2, 1, 1);

        Self {
            container,
            image_container,
            details_label,
            details_scrolled,
            current_loading_path: Arc::new(Mutex::new(None)),
            current_task_id: Arc::new(AtomicU64::new(0)),
            next_task_id: Arc::new(AtomicU64::new(1)),
            cache_dir,
        }
    }

    pub fn update_with_content(&self, item: &Item) {
        match item.source {
            crate::config::SourceMode::Dynamic => self.update_with_dynamic_content(item),
            _ => match item.display {
                crate::config::DisplayMode::Picture => self.update_with_image_content(item),
                crate::config::DisplayMode::Text => self.update_with_text_content(item),
            }
        }
    }

    fn update_with_dynamic_content(&self, item: &Item) {
        // For dynamic items, the value field contains the ID to use in the preview command
        // We need to execute the preview command with the ID to get the actual content

        let preview_cmd = format!("cliphist decode {}", item.value);

        match std::process::Command::new("sh").arg("-c").arg(&preview_cmd).output() {
            Ok(output) if output.status.success() => {
                // Check if the output contains binary data (contains null bytes or other control chars)
                let is_binary = output.stdout.iter().any(|&b| b == 0 || (b < 32 && b != b'\n' && b != b'\t'));

                if is_binary {
                    // Binary content - save to temp file and display as image
                    use std::fs::File;
                    use std::io::Write;
                    use tempfile::NamedTempFile;

                    // Create a temporary file to hold the binary data
                    let mut temp_file = match NamedTempFile::new() {
                        Ok(tf) => tf,
                        Err(_) => {
                            // If we can't create a temp file, show an error
                            let details_text = format!(
                                "<b>Title:</b> {}\n<b>Category:</b> {}\n<b>Error:</b> Could not create temporary file",
                                glib::markup_escape_text(&item.title),
                                glib::markup_escape_text(&item.category)
                            );
                            self.details_label.set_markup(&details_text);

                            while let Some(child) = self.image_container.first_child() {
                                self.image_container.remove(&child);
                            }

                            let error_label = gtk4::Label::new(Some("Could not create temporary file for image preview"));
                            error_label.set_halign(gtk4::Align::Center);
                            error_label.set_valign(gtk4::Align::Center);
                            error_label.set_hexpand(true);
                            error_label.set_vexpand(true);
                            error_label.add_css_class("error-label");

                            self.image_container.append(&error_label);
                            return;
                        }
                    };

                    if temp_file.write_all(&output.stdout).is_ok() {
                        let path_str = temp_file.path().to_string_lossy().to_string();

                        // Update details
                        let details_text = format!(
                            "<b>Title:</b> {}\n<b>Category:</b> {}\n<b>Path:</b> {}",
                            glib::markup_escape_text(&item.title),
                            glib::markup_escape_text(&item.category),
                            glib::markup_escape_text(&path_str)
                        );
                        self.details_label.set_markup(&details_text);

                        let adjustment = self.details_scrolled.vadjustment();
                        adjustment.set_value(0.0);

                        // Clear previous content
                        while let Some(child) = self.image_container.first_child() {
                            self.image_container.remove(&child);
                        }

                        // Load image from the temporary file
                        match gdk_pixbuf::Pixbuf::from_file_at_scale(
                            temp_file.path(),
                            crate::constants::IMAGE_PREVIEW_WIDTH,
                            crate::constants::IMAGE_PREVIEW_HEIGHT,
                            true
                        ) {
                            Ok(texture) => {
                                let picture = gtk4::Picture::for_pixbuf(&texture);
                                picture.set_halign(gtk4::Align::Center);
                                picture.set_valign(gtk4::Align::Center);
                                picture.set_hexpand(true);
                                picture.set_vexpand(true);
                                picture.add_css_class("preview-image");

                                self.image_container.append(&picture);

                                // Keep the temp file alive for the duration of the preview
                                std::mem::forget(temp_file);
                            },
                            Err(_) => {
                                let error_label = gtk4::Label::new(Some(&format!("Failed to load image from temp file:\n{}", path_str)));
                                error_label.set_halign(gtk4::Align::Center);
                                error_label.set_valign(gtk4::Align::Center);
                                error_label.set_hexpand(true);
                                error_label.set_vexpand(true);
                                error_label.add_css_class("error-label");

                                self.image_container.append(&error_label);
                            }
                        }
                    } else {
                        // Failed to write to temp file
                        let error_label = gtk4::Label::new(Some("Failed to write image data to temporary file"));
                        error_label.set_halign(gtk4::Align::Center);
                        error_label.set_valign(gtk4::Align::Center);
                        error_label.set_hexpand(true);
                        error_label.set_vexpand(true);
                        error_label.add_css_class("error-label");

                        self.image_container.append(&error_label);
                    }
                } else {
                    // It's text content, display as text
                    let content = String::from_utf8_lossy(&output.stdout);
                    let details_text = format!(
                        "<b>Title:</b> {}\n<b>Category:</b> {}\n<b>Value:</b> {}",
                        glib::markup_escape_text(&item.title),
                        glib::markup_escape_text(&item.category),
                        glib::markup_escape_text(&content)
                    );
                    self.details_label.set_markup(&details_text);

                    self.details_label
                        .set_ellipsize(gtk4::pango::EllipsizeMode::End);

                    let adjustment = self.details_scrolled.vadjustment();
                    adjustment.set_value(0.0);

                    // Clear previous content
                    while let Some(child) = self.image_container.first_child() {
                        self.image_container.remove(&child);
                    }

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
                    text_view.set_vexpand(false);  // Don't expand vertically
                    text_view.set_size_request(-1, 200);  // Limit height

                    let scrolled_window = gtk4::ScrolledWindow::new();
                    scrolled_window.set_child(Some(&text_view));
                    scrolled_window.set_hexpand(true);
                    scrolled_window.set_vexpand(false);  // Don't expand vertically
                    scrolled_window.set_size_request(-1, 200);  // Limit height

                    scrolled_window.set_hscrollbar_policy(gtk4::PolicyType::Automatic);
                    scrolled_window.set_vscrollbar_policy(gtk4::PolicyType::Automatic);

                    self.image_container.append(&scrolled_window);
                }
            }
            _ => {
                let details_text = format!(
                    "<b>Title:</b> {}\n<b>Category:</b> {}\n<b>Error:</b> Failed to execute preview command",
                    glib::markup_escape_text(&item.title),
                    glib::markup_escape_text(&item.category)
                );
                self.details_label.set_markup(&details_text);

                // Clear previous content
                while let Some(child) = self.image_container.first_child() {
                    self.image_container.remove(&child);
                }

                let error_label = gtk4::Label::new(Some("Failed to execute preview command"));
                error_label.set_halign(gtk4::Align::Center);
                error_label.set_valign(gtk4::Align::Center);
                error_label.set_hexpand(true);
                error_label.set_vexpand(true);
                error_label.add_css_class("error-label");

                self.image_container.append(&error_label);
            }
        }
    }

    fn update_with_image_content(&self, item: &Item) {
        // Check if the value is actually a file path that exists
        let path = std::path::Path::new(&item.value);
        if !path.exists() || !path.is_file() {
            // Value is not a valid file path, treat as text content
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
                let image_container_clone = self.image_container.clone();
                let path_str_clone = path_str.clone();

                move || {
                    while let Some(child) = image_container_clone.first_child() {
                        image_container_clone.remove(&child);
                    }

                    let error_label =
                        Label::new(Some(&format!("File does not exist:\n{}", path_str_clone)));
                    error_label.set_halign(Align::Center);
                    error_label.set_valign(Align::Center);
                    error_label.set_hexpand(true);
                    error_label.set_vexpand(true);
                    image_container_clone.append(&error_label);
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

        if cache_path.exists() {
            if let (Ok(original_modified), Ok(cache_metadata)) = (
                expanded_path.metadata().and_then(|m| m.modified()),
                cache_path.metadata().and_then(|m| m.modified()),
            ) {
                if cache_metadata >= original_modified {
                    let pixbuf_result = Pixbuf::from_file_at_scale(&cache_path, 800, 600, true);

                    glib::idle_add_local({
                        let image_container_clone = self.image_container.clone();
                        let current_loading_path_clone = self.current_loading_path.clone();
                        let path_str_clone = path_str.clone();
                        let current_task_id_clone = self.current_task_id.clone();

                        move || {
                            if current_task_id_clone.load(Ordering::SeqCst) != task_id {
                                return glib::ControlFlow::Break;
                            }

                            while let Some(child) = image_container_clone.first_child() {
                                image_container_clone.remove(&child);
                            }

                            if let Ok(ref pixbuf) = pixbuf_result {
                                let picture = Picture::for_pixbuf(pixbuf);

                                picture.set_halign(Align::Center);
                                picture.set_valign(Align::Center);
                                picture.set_hexpand(true);
                                picture.set_vexpand(true);

                                image_container_clone.append(&picture);
                            } else {
                                let error_label = Label::new(Some("Failed to load image"));
                                error_label.set_halign(Align::Center);
                                error_label.set_valign(Align::Center);
                                error_label.set_hexpand(true);
                                error_label.set_vexpand(true);
                                image_container_clone.append(&error_label);
                            }

                            if let Ok(mut current_path) = current_loading_path_clone.lock() {
                                if current_path.as_ref() == Some(&path_str_clone) {
                                    *current_path = None;
                                }
                            }

                            glib::ControlFlow::Break
                        }
                    });
                    return;
                }
            }
        }

        let image_container_clone_for_async = self.image_container.clone();
        let current_loading_path_clone = self.current_loading_path.clone();
        let current_task_id_clone = self.current_task_id.clone();
        let cache_path_clone = cache_path.clone();
        let expanded_path_clone = expanded_path.clone();

        while let Some(child) = self.image_container.first_child() {
            self.image_container.remove(&child);
        }

        let loading_label = Label::new(Some("Loading..."));
        loading_label.set_halign(Align::Center);
        loading_label.set_valign(Align::Center);
        loading_label.set_hexpand(true);
        loading_label.set_vexpand(true);
        self.image_container.append(&loading_label);

        glib::spawn_future_local(async move {
            if current_task_id_clone.load(Ordering::SeqCst) != task_id {
                return;
            }

            let result = Pixbuf::from_file_at_scale(&expanded_path_clone, 800, 600, true);

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

                while let Some(child) = image_container_clone_for_async.first_child() {
                    image_container_clone_for_async.remove(&child);
                }

                if let Ok(ref pixbuf) = pixbuf_result {
                    let picture = Picture::for_pixbuf(pixbuf);

                    picture.set_halign(Align::Center);
                    picture.set_valign(Align::Center);
                    picture.set_hexpand(true);
                    picture.set_vexpand(true);

                    image_container_clone_for_async.append(&picture);
                } else {
                    let error_label = Label::new(Some("Failed to load image"));
                    error_label.set_halign(Align::Center);
                    error_label.set_valign(Align::Center);
                    error_label.set_hexpand(true);
                    error_label.set_vexpand(true);
                    image_container_clone_for_async.append(&error_label);
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
        let display_value = if item.value.chars().count() > 100 {
            let mut truncated = String::new();
            for (i, ch) in item.value.chars().enumerate() {
                if i >= 100 {
                    truncated.push_str("...");
                    break;
                }
                truncated.push(ch);
            }
            truncated
        } else {
            item.value.clone()
        };

        let details_text = format!(
            "<b>Title:</b> {}\n<b>Category:</b> {}\n<b>Value:</b> {}",
            glib::markup_escape_text(&item.title),
            glib::markup_escape_text(&item.category),
            glib::markup_escape_text(&display_value)
        );
        self.details_label.set_markup(&details_text);

        self.details_label
            .set_ellipsize(gtk4::pango::EllipsizeMode::End);

        let adjustment = self.details_scrolled.vadjustment();
        adjustment.set_value(0.0);

        while let Some(child) = self.image_container.first_child() {
            self.image_container.remove(&child);
        }

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

        self.image_container.append(&scrolled_window);
    }

    pub fn clear(&self) {
        while let Some(child) = self.image_container.first_child() {
            self.image_container.remove(&child);
        }
        self.details_label.set_text("No image selected");

        if let Ok(mut current_path) = self.current_loading_path.lock() {
            *current_path = None;
        }
    }
}
