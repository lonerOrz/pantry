use std::path::{Path, PathBuf};

pub fn is_path_directory(path: &str) -> bool {
    let expanded_path = expand_tilde(path);
    Path::new(&expanded_path).is_dir()
}

pub fn get_file_type(path: &str) -> FileType {
    let lower_path = path.to_lowercase();
    if lower_path == "file" || lower_path.contains("file:") {
        return FileType::File;
    }

    if path.starts_with('/') || path.starts_with("~/") {
        let expanded_path = expand_tilde(path);
        if let Ok(file_content) = std::fs::read(&expanded_path) {
            if let Some(kind) = infer::get(&file_content) {
                return match kind.mime_type() {
                    m if m.starts_with("image/") => FileType::Image,
                    m if m.starts_with("video/") => FileType::Video,
                    m if m.starts_with("text/") => FileType::Text,
                    m if m.starts_with("application/") => FileType::Binary,
                    _ => FileType::Other,
                };
            }
        }
    }

    match lower_path.rsplit_once('.') {
        Some((_, ext)) => match ext {
            "jpg" | "jpeg" | "png" | "gif" | "bmp" | "webp" => FileType::Image,
            "mp4" | "avi" | "mov" | "wmv" | "flv" | "mkv" | "webm" | "m4v" => FileType::Video,
            "txt" | "md" | "csv" | "json" | "xml" | "yaml" | "toml" => FileType::Text,
            _ => FileType::Other,
        },
        None => FileType::Other,
    }
}

#[derive(Debug, PartialEq)]
pub enum FileType {
    Image,
    Video,
    Text,
    Binary,
    File,
    Other,
}

pub fn expand_tilde<P: AsRef<Path>>(path: P) -> PathBuf {
    let path = path.as_ref();
    if path.starts_with("~") {
        if path == Path::new("~") {
            dirs::home_dir().unwrap_or_else(|| PathBuf::from("."))
        } else {
            dirs::home_dir()
                .map(|mut home| {
                    home.push(path.strip_prefix("~/").unwrap_or(path));
                    home
                })
                .unwrap_or_else(|| PathBuf::from("."))
        }
    } else {
        path.to_path_buf()
    }
}

pub fn path_to_safe_filename<P: AsRef<Path>>(path: P) -> String {
    let path_str = path.as_ref().to_string_lossy();
    path_str
        .chars()
        .map(|c| match c {
            '/' | '\\' | ':' | '*' | '?' | '"' | '<' | '>' | '|' => '_',
            c if c.is_control() => '_',
            c => c,
        })
        .collect()
}
