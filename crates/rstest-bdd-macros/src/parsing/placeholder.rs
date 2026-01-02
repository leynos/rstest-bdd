//! Placeholder substitution for Scenario Outline step text.
//!
//! This module provides utilities for substituting `<placeholder>` tokens in
//! Gherkin step text with values from an Examples table row. This enables the
//! cucumber-rs-style parameterisation where `Given I have <count> items` with
//! an Examples row `| count | = | 5 |` becomes `Given I have 5 items`.

use regex::Regex;
use std::sync::LazyLock;
use thiserror::Error;

/// Regex pattern matching `<placeholder>` tokens in step text.
///
/// Captures the placeholder name without the angle brackets, including spaces
/// and punctuation commonly used in Gherkin Examples headers.
pub(crate) static PLACEHOLDER_RE: LazyLock<Regex> = LazyLock::new(|| {
    // Safe: The regex pattern is a compile-time constant and is valid.
    Regex::new(r"<([^>\s][^>]*)>").unwrap_or_else(|_| unreachable!("placeholder regex is valid"))
});

/// Error returned when a placeholder references a non-existent column.
#[derive(Debug, Clone, PartialEq, Eq, Error)]
#[error(
    "Placeholder '<{placeholder}>' not found in Examples table. Available columns: \
     [{available_columns_display}]"
)]
pub struct PlaceholderError {
    /// The placeholder name that was not found.
    pub placeholder: String,
    /// The available column headers in the Examples table.
    pub available_columns: Vec<String>,
    available_columns_display: String,
}

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
                available_columns_display: headers.join(", "),
            })?;

        // Invariant: headers and row must have equal length; panic if violated.
        let value = row.get(idx).unwrap_or_else(|| {
            panic!("row length must match headers length; this indicates a parsing bug");
        });
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
    use rstest::rstest;

    #[rstest]
    #[case(
        "I have <count> items",
        vec!["count".to_string()],
        vec!["5".to_string()],
        "I have 5 items"
    )]
    #[case(
        "I have <count> <item>",
        vec!["count".to_string(), "item".to_string()],
        vec!["5".to_string(), "apples".to_string()],
        "I have 5 apples"
    )]
    #[case(
        "<val> plus <val> equals double <val>",
        vec!["val".to_string()],
        vec!["3".to_string()],
        "3 plus 3 equals double 3"
    )]
    #[case(
        "I have 5 items",
        vec!["count".to_string()],
        vec!["10".to_string()],
        "I have 5 items"
    )]
    #[case(
        "I have <count> items",
        vec!["count".to_string()],
        vec![String::new()],
        "I have  items"
    )]
    fn substitute_placeholders_cases(
        #[case] text: &str,
        #[case] headers: Vec<String>,
        #[case] row: Vec<String>,
        #[case] expected: &str,
    ) {
        let result = substitute_placeholders(text, &headers, &row).unwrap();
        assert_eq!(result, expected);
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
    fn placeholder_with_spaces_and_hyphens() {
        let text = "The <start count> includes <item-id>";
        let headers = vec!["start count".to_string(), "item-id".to_string()];
        let row = vec!["3".to_string(), "apples".to_string()];

        let result = substitute_placeholders(text, &headers, &row).unwrap();
        assert_eq!(result, "The 3 includes apples");
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
