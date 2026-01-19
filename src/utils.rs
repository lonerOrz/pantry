use std::path::{Path, PathBuf};

/// 检查路径是否为目录
pub fn is_path_directory(path: &str) -> bool {
    let expanded_path = expand_tilde(path);
    std::path::Path::new(&expanded_path).is_dir()
}

/// 展开路径中的波浪号 (~) 为家目录
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

/// 将路径转换为安全的文件名（替换非法字符）
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
