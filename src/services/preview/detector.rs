pub fn is_binary(data: &[u8]) -> bool {
    data.iter()
        .any(|&b| b == 0 || (b < 32 && b != b'\n' && b != b'\t'))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn plain_text_not_binary() {
        assert!(!is_binary(b"Hello World!\nLine 2\tTabbed"));
    }

    #[test]
    fn null_bytes_is_binary() {
        assert!(is_binary(&[0x00, 0x01, 0x02]));
    }

    #[test]
    fn printable_with_newlines_not_binary() {
        assert!(!is_binary(b"line1\nline2\nline3\n"));
    }
}
