use std::path::{Path, PathBuf};

pub fn is_path_directory(path: &str) -> bool {
    // Expand ~ to user's home directory
    let expanded_path = expand_tilde(path);
    Path::new(&expanded_path).is_dir()
}

pub fn is_image_file(path: &str) -> bool {
    let path_lower = path.to_lowercase();
    path_lower.ends_with(".jpg")
        || path_lower.ends_with(".jpeg")
        || path_lower.ends_with(".png")
        || path_lower.ends_with(".gif")
        || path_lower.ends_with(".bmp")
        || path_lower.ends_with(".webp")
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