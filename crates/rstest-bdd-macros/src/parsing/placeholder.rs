//! Placeholder substitution for Scenario Outline step text.
//!
//! This module provides utilities for substituting `<placeholder>` tokens in
//! Gherkin step text with values from an Examples table row. This enables the
//! cucumber-rs-style parameterisation where `Given I have <count> items` with
//! an Examples row `| count | = | 5 |` becomes `Given I have 5 items`.

use regex::Regex;
use std::fmt;
use std::sync::LazyLock;

/// Regex pattern matching `<placeholder>` tokens in step text.
///
/// Captures the placeholder name (alphanumeric plus underscores) without the
/// angle brackets.
pub(crate) static PLACEHOLDER_RE: LazyLock<Regex> = LazyLock::new(|| {
    // Safe: The regex pattern is a compile-time constant and is valid.
    Regex::new(r"<(\w+)>").unwrap_or_else(|_| unreachable!("placeholder regex is valid"))
});

/// Error returned when a placeholder references a non-existent column.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PlaceholderError {
    /// The placeholder name that was not found.
    pub placeholder: String,
    /// The available column headers in the Examples table.
    pub available_columns: Vec<String>,
}

impl fmt::Display for PlaceholderError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            f,
            "Placeholder '<{}>' not found in Examples table. Available columns: [{}]",
            self.placeholder,
            self.available_columns.join(", ")
        )
    }
}

impl std::error::Error for PlaceholderError {}

/// Substitutes `<placeholder>` tokens in text with values from an Examples row.
///
/// Each placeholder token (e.g., `<count>`) is replaced with the corresponding
/// value from the row, matched by column header name.
///
/// # Arguments
///
/// * `text` - The text containing placeholders to substitute
/// * `headers` - Column headers from the Examples table
/// * `row` - Values for the current row, aligned with headers
///
/// # Returns
///
/// The text with all placeholders substituted, or an error if any placeholder
/// references a column that doesn't exist in the headers.
///
/// # Examples
///
/// ```ignore
/// let text = "I have <count> <item>";
/// let headers = vec!["count".to_string(), "item".to_string()];
/// let row = vec!["5".to_string(), "apples".to_string()];
/// let result = substitute_placeholders(text, &headers, &row);
/// assert_eq!(result.unwrap(), "I have 5 apples");
/// ```
pub fn substitute_placeholders(
    text: &str,
    headers: &[String],
    row: &[String],
) -> Result<String, PlaceholderError> {
    let mut result = text.to_string();

    for cap in PLACEHOLDER_RE.captures_iter(text) {
        // Safe: capture group 0 always exists for a successful match.
        let Some(full_match) = cap.get(0) else {
            continue;
        };
        let placeholder = &cap[1];

        let idx = headers
            .iter()
            .position(|h| h == placeholder)
            .ok_or_else(|| PlaceholderError {
                placeholder: placeholder.to_string(),
                available_columns: headers.to_vec(),
            })?;

        // Safe: idx is derived from the headers which must match row in length.
        let Some(value) = row.get(idx) else {
            continue;
        };
        result = result.replace(full_match.as_str(), value);
    }

    Ok(result)
}

/// Checks if a text contains any placeholder tokens.
///
/// Returns `true` if the text contains at least one `<placeholder>` pattern.
pub fn contains_placeholders(text: &str) -> bool {
    PLACEHOLDER_RE.is_match(text)
}

/// Extracts all placeholder names from a text.
///
/// Returns a vector of placeholder names (without angle brackets) found in the
/// text, preserving the order of first occurrence.
#[cfg(test)]
fn extract_placeholder_names(text: &str) -> Vec<String> {
    PLACEHOLDER_RE
        .captures_iter(text)
        .map(|cap| cap[1].to_string())
        .collect()
}

#[cfg(test)]
#[expect(clippy::unwrap_used, reason = "tests use unwrap for brevity")]
mod tests {
    use super::*;

    #[test]
    fn substitute_single_placeholder() {
        let text = "I have <count> items";
        let headers = vec!["count".to_string()];
        let row = vec!["5".to_string()];

        let result = substitute_placeholders(text, &headers, &row).unwrap();
        assert_eq!(result, "I have 5 items");
    }

    #[test]
    fn substitute_multiple_placeholders() {
        let text = "I have <count> <item>";
        let headers = vec!["count".to_string(), "item".to_string()];
        let row = vec!["5".to_string(), "apples".to_string()];

        let result = substitute_placeholders(text, &headers, &row).unwrap();
        assert_eq!(result, "I have 5 apples");
    }

    #[test]
    fn substitute_repeated_placeholder() {
        let text = "<val> plus <val> equals double <val>";
        let headers = vec!["val".to_string()];
        let row = vec!["3".to_string()];

        let result = substitute_placeholders(text, &headers, &row).unwrap();
        assert_eq!(result, "3 plus 3 equals double 3");
    }

    #[test]
    fn substitute_no_placeholders() {
        let text = "I have 5 items";
        let headers = vec!["count".to_string()];
        let row = vec!["10".to_string()];

        let result = substitute_placeholders(text, &headers, &row).unwrap();
        assert_eq!(result, "I have 5 items");
    }

    #[test]
    fn substitute_empty_value() {
        let text = "I have <count> items";
        let headers = vec!["count".to_string()];
        let row = vec![String::new()];

        let result = substitute_placeholders(text, &headers, &row).unwrap();
        assert_eq!(result, "I have  items");
    }

    #[test]
    fn error_on_undefined_placeholder() {
        let text = "I have <undefined> items";
        let headers = vec!["count".to_string()];
        let row = vec!["5".to_string()];

        let result = substitute_placeholders(text, &headers, &row);
        assert!(result.is_err());

        let err = result.unwrap_err();
        assert_eq!(err.placeholder, "undefined");
        assert_eq!(err.available_columns, vec!["count".to_string()]);
    }

    #[test]
    fn placeholder_with_underscores() {
        let text = "The <user_name> has <item_count> items";
        let headers = vec!["user_name".to_string(), "item_count".to_string()];
        let row = vec!["Alice".to_string(), "42".to_string()];

        let result = substitute_placeholders(text, &headers, &row).unwrap();
        assert_eq!(result, "The Alice has 42 items");
    }

    #[test]
    fn contains_placeholders_returns_true() {
        assert!(contains_placeholders("I have <count> items"));
        assert!(contains_placeholders("<a> and <b>"));
    }

    #[test]
    fn contains_placeholders_returns_false() {
        assert!(!contains_placeholders("I have 5 items"));
        assert!(!contains_placeholders("No placeholders here"));
        assert!(!contains_placeholders("Angle brackets < > without names"));
    }

    #[test]
    fn extract_placeholder_names_finds_all() {
        let names = extract_placeholder_names("I have <count> <item> and <count> more");
        assert_eq!(names, vec!["count", "item", "count"]);
    }

    #[test]
    fn extract_placeholder_names_empty_when_none() {
        let names = extract_placeholder_names("No placeholders here");
        assert!(names.is_empty());
    }
}
