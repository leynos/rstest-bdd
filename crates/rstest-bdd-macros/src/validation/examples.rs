//! Validation routines for example tables in features.

use crate::utils::errors::error_to_tokens;
use proc_macro2::TokenStream;

/// Validate Examples table structure in feature file text.
#[cfg(feature = "compile-time-validation")]
pub(crate) fn validate_examples_in_feature_text(text: &str) -> Result<(), TokenStream> {
    if !text.contains("Examples:") {
        return Ok(());
    }

    let examples_idx = find_examples_table_start(text)?;
    validate_table_column_consistency(text, examples_idx)
}

#[cfg(feature = "compile-time-validation")]
fn find_examples_table_start(text: &str) -> Result<usize, TokenStream> {
    text.lines()
        .enumerate()
        .find(|(_, line)| line.trim_start().starts_with("Examples:"))
        .map(|(idx, _)| idx)
        .ok_or_else(|| {
            error_to_tokens(&syn::Error::new(
                proc_macro2::Span::call_site(),
                "Examples table structure error",
            ))
        })
}

#[cfg(feature = "compile-time-validation")]
fn validate_table_column_consistency(text: &str, start_idx: usize) -> Result<(), TokenStream> {
    let mut table_rows = text
        .lines()
        .skip(start_idx + 1)
        .take_while(|line| line.trim_start().starts_with('|'));

    let Some(header_row) = table_rows.next() else {
        return Ok(());
    };

    let expected_columns = count_columns(header_row);

    for (i, data_row) in table_rows.enumerate() {
        let actual_columns = count_columns(data_row);
        if actual_columns != expected_columns {
            let msg = format!(
                "Malformed Examples table: row {} has {} columns, expected {}",
                i + 2,
                actual_columns,
                expected_columns
            );
            return Err(error_to_tokens(&syn::Error::new(
                proc_macro2::Span::call_site(),
                msg,
            )));
        }
    }

    Ok(())
}

#[cfg(feature = "compile-time-validation")]
fn count_columns(row: &str) -> usize {
    // Count pipe-delimited segments; subtract the leading segment before the
    // first pipe to align with Gherkin column counting semantics used
    // elsewhere in the codebase.
    row.split('|').count().saturating_sub(1)
}

#[cfg(all(test, feature = "compile-time-validation"))]
mod column_tests {
    //! Tests for column counting in Examples tables.
    use super::count_columns;
    use rstest::rstest;

    #[rstest]
    #[case("| a | b |", 3)]
    #[case("a|b", 1)]
    #[case("| value |", 2)]
    #[case("||", 2)]
    #[case("", 0)]
    #[case("| |", 2)]
    fn counts_columns(#[case] row: &str, #[case] expected: usize) {
        assert_eq!(count_columns(row), expected);
    }
}

pub(crate) fn extract_and_validate_headers(
    table: &gherkin::Table,
) -> Result<Vec<String>, TokenStream> {
    let first = table.rows.first().ok_or_else(|| {
        error_to_tokens(&syn::Error::new(
            proc_macro2::Span::call_site(),
            "Examples table must have at least one row",
        ))
    })?;
    Ok(first.clone())
}

pub(crate) fn validate_header_consistency(
    scenario: &gherkin::Scenario,
    expected_headers: &[String],
) -> Result<(), TokenStream> {
    for ex in scenario.examples.iter().skip(1) {
        let table = ex.table.as_ref().ok_or_else(|| {
            error_to_tokens(&syn::Error::new(
                proc_macro2::Span::call_site(),
                "Examples table missing rows",
            ))
        })?;
        let headers = table.rows.first().ok_or_else(|| {
            error_to_tokens(&syn::Error::new(
                proc_macro2::Span::call_site(),
                "Examples table must have at least one row",
            ))
        })?;
        if headers != expected_headers {
            return Err(error_to_tokens(&syn::Error::new(
                proc_macro2::Span::call_site(),
                "All Examples tables must have the same headers",
            )));
        }
    }
    Ok(())
}

pub(crate) fn flatten_and_validate_rows(
    scenario: &gherkin::Scenario,
    expected_width: usize,
) -> Result<Vec<Vec<String>>, TokenStream> {
    let rows: Vec<Vec<String>> = scenario
        .examples
        .iter()
        .filter_map(|ex| ex.table.as_ref())
        .flat_map(|t| t.rows.iter().skip(1).cloned())
        .collect();

    for (i, row) in rows.iter().enumerate() {
        if row.len() != expected_width {
            let err = syn::Error::new(
                proc_macro2::Span::call_site(),
                format!(
                    "Malformed examples table: row {} has {} columns, expected {}",
                    i + 2,
                    row.len(),
                    expected_width
                ),
            );
            return Err(error_to_tokens(&err));
        }
    }

    Ok(rows)
}

#[cfg(all(test, feature = "compile-time-validation"))]
mod tests {
    use super::*;
    use rstest::rstest;

    fn error_message(text: &str) -> String {
        match validate_examples_in_feature_text(text) {
            Ok(()) => panic!("expected validation to fail"),
            Err(err) => err.to_string(),
        }
    }

    fn assert_column_mismatch_error(text: &str, expected_error_substring: &str) {
        let msg = error_message(text);
        assert!(
            msg.contains(expected_error_substring),
            "unexpected error: {msg}"
        );
    }

    #[test]
    fn accepts_matching_columns() {
        let text = "\
Examples:
| a | b |
| 1 | 2 |
| 3 | 4 |
";

        assert!(validate_examples_in_feature_text(text).is_ok());
    }

    #[rstest]
    #[case(
        "\
Examples:
| a | b |
| 1 | 2 |
| 3 | 4 | 5 |
",
        "Malformed Examples table: row 3 has 4 columns, expected 3"
    )]
    #[case(
        "\
Examples:
| a | b | c |
| 1 | 2 | 3 |
| 4 | 5 |
",
        "Malformed Examples table: row 3 has 3 columns, expected 4"
    )]
    fn reports_column_mismatch(#[case] text: &str, #[case] expected_error: &str) {
        assert_column_mismatch_error(text, expected_error);
    }
}
