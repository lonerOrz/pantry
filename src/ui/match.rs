pub fn fuzzy_match(text: &str, pattern: &str) -> bool {
    let mut pattern_chars = pattern.chars();
    let Some(mut expected) = pattern_chars.next() else {
        return true;
    };

    for c in text.chars() {
        if c == expected {
            match pattern_chars.next() {
                Some(next) => expected = next,
                None => return true,
            }
        }
    }

    false
}

pub fn relevance_score(title: &str, value: &str, query: &str) -> Option<i32> {
    let query_lower = query.to_lowercase();
    let title_lower = title.to_lowercase();
    let value_lower = value.to_lowercase();

    if title_lower == query_lower {
        Some(100)
    } else if title_lower.starts_with(&query_lower) {
        Some(60)
    } else if title_lower.contains(&query_lower) {
        Some(30)
    } else if value_lower.contains(&query_lower) {
        Some(20)
    } else if fuzzy_match(&title_lower, &query_lower) {
        Some(10)
    } else {
        None
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn fuzzy_empty_pattern_matches() {
        assert!(fuzzy_match("hello", ""));
    }

    #[test]
    fn fuzzy_exact_match() {
        assert!(fuzzy_match("hello", "hello"));
    }

    #[test]
    fn fuzzy_subsequence_match() {
        assert!(fuzzy_match("hello", "hlo"));
    }

    #[test]
    fn fuzzy_no_match() {
        assert!(!fuzzy_match("hello", "xyz"));
    }

    #[test]
    fn fuzzy_case_sensitive() {
        assert!(!fuzzy_match("Hello", "hello"));
    }

    #[test]
    fn fuzzy_single_char() {
        assert!(fuzzy_match("abc", "a"));
        assert!(fuzzy_match("abc", "c"));
        assert!(!fuzzy_match("abc", "d"));
    }

    #[test]
    fn relevance_exact_title() {
        assert_eq!(relevance_score("foo", "bar", "foo"), Some(100));
    }

    #[test]
    fn relevance_title_starts_with() {
        assert_eq!(relevance_score("foobar", "x", "foo"), Some(60));
    }

    #[test]
    fn relevance_title_contains() {
        assert_eq!(relevance_score("foobar", "x", "oob"), Some(30));
    }

    #[test]
    fn relevance_value_contains() {
        assert_eq!(relevance_score("xyz", "hello world", "hello"), Some(20));
    }

    #[test]
    fn relevance_fuzzy() {
        assert_eq!(relevance_score("fxbztest", "x", "fb"), Some(10));
    }

    #[test]
    fn relevance_no_match() {
        assert_eq!(relevance_score("xyz", "abc", "hello"), None);
    }

    #[test]
    fn relevance_case_insensitive() {
        assert_eq!(relevance_score("Hello", "x", "hello"), Some(100));
    }
}
