//! Regex capture helpers shared by compile-time and runtime crates.

use regex::Regex;

/// Extract capture-group values for user placeholders from a compiled regular expression.
///
/// Returns `None` if `text` does not match. Capture group 0 (the whole match) is ignored.
/// Optional groups that do not participate yield empty strings.
///
/// # Examples
/// ```
/// # use regex::Regex;
/// # use rstest_bdd_patterns::extract_captured_values;
/// let regex = Regex::new(r"^(\d+)-(\w+)$")
///     .expect("example ensures fallible call succeeds");
/// let values = extract_captured_values(&regex, "42-answer")
///     .expect("example ensures fallible call succeeds");
/// assert_eq!(values, vec!["42".to_string(), "answer".to_string()]);
/// ```
#[must_use]
pub fn extract_captured_values(re: &Regex, text: &str) -> Option<Vec<String>> {
    let caps = re.captures(text)?;
    let mut values = Vec::with_capacity(caps.len().saturating_sub(1));
    for capture in caps.iter().skip(1) {
        let value = capture.map_or_else(String::new, |m| m.as_str().to_string());
        values.push(value);
    }

    Some(values)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn returns_none_when_pattern_does_not_match() {
        let regex = Regex::new(r"^(\d+)$").unwrap_or_else(|err| panic!("valid regex: {err}"));
        assert!(extract_captured_values(&regex, "nope").is_none());
    }

    #[test]
    fn collects_captures_in_order() {
        let regex =
            Regex::new(r"^(\d+)-(\w+)-(\d+)$").unwrap_or_else(|err| panic!("valid regex: {err}"));
        let captures = extract_captured_values(&regex, "12-answer-7")
            .unwrap_or_else(|| panic!("expected a match"));
        assert_eq!(captures, vec!["12", "answer", "7"]);
    }

    #[test]
    fn supports_empty_optional_groups() {
        let regex = Regex::new(r"^(a)?(b)?$").unwrap_or_else(|err| panic!("valid regex: {err}"));
        let captures =
            extract_captured_values(&regex, "a").unwrap_or_else(|| panic!("expected a match"));
        assert_eq!(captures, vec![String::from("a"), String::new()]);
    }
}
