use std::path::{Path, PathBuf};

pub fn is_path_directory(path: &str) -> bool {
    let expanded_path = expand_tilde(path);
    Path::new(&expanded_path).is_dir()
}

pub fn is_image_file(path: &str) -> bool {
    if path.starts_with('/') || path.starts_with("~/") {
        let expanded_path = expand_tilde(path);
        if let Ok(file_content) = std::fs::read(&expanded_path) {
            if let Some(kind) = infer::get(&file_content) {
                return kind.mime_type().starts_with("image/");
            }
        }
    }

    // Fallback to extension-based detection
    let path_lower = path.to_lowercase();
    path_lower.ends_with(".jpg")
        || path_lower.ends_with(".jpeg")
        || path_lower.ends_with(".png")
        || path_lower.ends_with(".gif")
        || path_lower.ends_with(".bmp")
        || path_lower.ends_with(".webp")
}

pub fn is_video_file(path: &str) -> bool {
    if path.starts_with('/') || path.starts_with("~/") {
        let expanded_path = expand_tilde(path);
        if let Ok(file_content) = std::fs::read(&expanded_path) {
            if let Some(kind) = infer::get(&file_content) {
                return kind.mime_type().starts_with("video/");
            }
        }
    }

    // Fallback to extension-based detection
    let path_lower = path.to_lowercase();
    path_lower.ends_with(".mp4")
        || path_lower.ends_with(".avi")
        || path_lower.ends_with(".mov")
        || path_lower.ends_with(".wmv")
        || path_lower.ends_with(".flv")
        || path_lower.ends_with(".mkv")
        || path_lower.ends_with(".webm")
        || path_lower.ends_with(".m4v")
}

pub fn get_file_type(path: &str) -> FileType {
    // 如果路径看起来像是文件路径，尝试检测其内容类型
    if path.starts_with('/') || path.starts_with("~/") {
        let expanded_path = expand_tilde(path);
        if let Ok(file_content) = std::fs::read(&expanded_path) {
            if let Some(kind) = infer::get(&file_content) {
                let mime_type = kind.mime_type();
                if mime_type.starts_with("image/") {
                    return FileType::Image;
                } else if mime_type.starts_with("video/") {
                    return FileType::Video;
                } else if mime_type.starts_with("text/") {
                    return FileType::Text;
                } else if mime_type.starts_with("application/") {
                    return FileType::Binary;
                } else {
                    return FileType::Other;
                }
            }
        }
    }

    // Fallback to extension-based detection
    let path_lower = path.to_lowercase();
    if path_lower.ends_with(".jpg")
        || path_lower.ends_with(".jpeg")
        || path_lower.ends_with(".png")
        || path_lower.ends_with(".gif")
        || path_lower.ends_with(".bmp")
        || path_lower.ends_with(".webp") {
        FileType::Image
    } else if path_lower.ends_with(".mp4")
        || path_lower.ends_with(".avi")
        || path_lower.ends_with(".mov")
        || path_lower.ends_with(".wmv")
        || path_lower.ends_with(".flv")
        || path_lower.ends_with(".mkv")
        || path_lower.ends_with(".webm")
        || path_lower.ends_with(".m4v") {
        FileType::Video
    } else if path_lower.ends_with(".txt")
        || path_lower.ends_with(".md")
        || path_lower.ends_with(".csv")
        || path_lower.ends_with(".json")
        || path_lower.ends_with(".xml")
        || path_lower.ends_with(".yaml")
        || path_lower.ends_with(".toml") {
        FileType::Text
    } else {
        FileType::Other
    }
}

#[derive(Debug, PartialEq)]
pub enum FileType {
    Image,
    Video,
    Text,
    Binary,
    Other,
}

pub fn expand_tilde<P: AsRef<Path>>(path: P) -> PathBuf {
    let path = path.as_ref();
    if path.starts_with("~") {
        if path == Path::new("~") {
            // Only tilde, return home directory
            dirs::home_dir().unwrap_or_else(|| PathBuf::from("."))
        } else {
            // Tilde followed by path, concatenate home directory and remaining path
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

/// Convert path to safe filename (replace unsafe characters)
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