use std::path::{Path, PathBuf};

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

pub fn escape_shell_arg(s: &str) -> String {
    if s.is_empty() {
        return "''".to_string();
    }
    let escaped = s.replace('\'', "'\\''");
    format!("'{}'", escaped)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn expand_tilde_with_home() {
        let result = expand_tilde("~/foo");
        let home = dirs::home_dir().unwrap();
        assert_eq!(result, home.join("foo"));
    }

    #[test]
    fn expand_tilde_without_home() {
        let result = expand_tilde("/tmp/foo");
        assert_eq!(result, PathBuf::from("/tmp/foo"));
    }

    #[test]
    fn expand_tilde_bare() {
        let result = expand_tilde("~");
        let home = dirs::home_dir().unwrap();
        assert_eq!(result, home);
    }

    #[test]
    fn safe_filename_replaces_illegal() {
        assert_eq!(path_to_safe_filename("a/b:c*d"), "a_b_c_d");
    }

    #[test]
    fn safe_filename_preserves_normal() {
        assert_eq!(path_to_safe_filename("hello.txt"), "hello.txt");
    }

    #[test]
    fn safe_filename_handles_control_chars() {
        assert_eq!(path_to_safe_filename("a\u{0000}b"), "a_b");
    }

    #[test]
    fn escape_plain_text() {
        assert_eq!(escape_shell_arg("hello"), "'hello'");
    }

    #[test]
    fn escape_injection_payload() {
        assert_eq!(escape_shell_arg("123; rm -rf /"), "'123; rm -rf /'");
    }

    #[test]
    fn escape_internal_single_quotes() {
        assert_eq!(escape_shell_arg("it's simple"), "'it'\\''s simple'");
    }

    #[test]
    fn escape_empty_string() {
        assert_eq!(escape_shell_arg(""), "''");
    }
}
