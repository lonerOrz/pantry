use std::path::{Path, PathBuf};

/// Check if path is a directory
pub fn is_path_directory(path: &str) -> bool {
    let expanded_path = expand_tilde(path);
    std::path::Path::new(&expanded_path).is_dir()
}

/// Expand tilde (~) in path to home directory
pub fn expand_tilde<P: AsRef<Path>>(path: P) -> PathBuf {
    let path = path.as_ref();
    if path.starts_with("~") {
        if path == std::path::Path::new("~") {
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

/// Convert path to safe filename (replace illegal characters)
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
