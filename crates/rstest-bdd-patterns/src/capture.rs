//! Regex capture helpers shared by compile-time and runtime crates.

use regex::Regex;

/// Extract the placeholder capture groups when `text` matches `re`, returning `None` otherwise.
///
/// This lets callers branch on a missing match instead of inspecting an empty capture
/// set. Capture group 0 (the full match) is ignored so only user-defined placeholders
/// contribute to the result, and optional placeholders that do not participate yield
/// empty strings to keep positional alignment.
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
///
/// ```
/// # use regex::Regex;
/// # use rstest_bdd_patterns::extract_captured_values;
/// let regex = Regex::new(r"^(\d+)$")
///     .expect("example ensures fallible call succeeds");
/// assert!(extract_captured_values(&regex, "nope").is_none());
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
        #[expect(
            clippy::expect_used,
            reason = "tests require descriptive panic messages"
        )]
        let regex = Regex::new(r"^(\d+)$").expect("test regex must compile");
        assert!(extract_captured_values(&regex, "nope").is_none());
    }

    #[test]
    fn collects_captures_in_order() {
        #[expect(
            clippy::expect_used,
            reason = "tests require descriptive panic messages"
        )]
        let regex = Regex::new(r"^(\d+)-(\w+)-(\d+)$").expect("test regex must compile");

        let input = "12-answer-7";
        let message = format!(
            "expected captures for input {input:?} using pattern {}",
            regex.as_str()
        );
        #[expect(
            clippy::expect_used,
            reason = "tests validate capture extraction succeeds"
        )]
        let captures = extract_captured_values(&regex, input).expect(message.as_str());
        assert_eq!(
            captures,
            vec![
                String::from("12"),
                String::from("answer"),
                String::from("7")
            ]
        );
    }

    #[test]
    fn supports_empty_optional_groups() {
        #[expect(
            clippy::expect_used,
            reason = "tests require descriptive panic messages"
        )]
        let regex = Regex::new(r"^(a)?(b)?$").expect("test regex must compile");

        let input = "a";
        let message = format!(
            "expected captures for input {input:?} using pattern {}",
            regex.as_str()
        );
        #[expect(
            clippy::expect_used,
            reason = "tests validate capture extraction succeeds"
        )]
        let captures = extract_captured_values(&regex, input).expect(message.as_str());
        assert_eq!(captures, vec![String::from("a"), String::new()]);
    }
}
