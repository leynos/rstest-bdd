//! Validates placeholder references in Scenario Outline steps.
//!
//! This module ensures that all `<placeholder>` tokens in step text, docstrings,
//! and data tables reference columns that exist in the Examples table. Validation
//! happens at compile time during macro expansion, providing early feedback for
//! misconfigured scenario outlines.

use crate::parsing::feature::ParsedStep;
use crate::parsing::placeholder::PLACEHOLDER_RE;
use proc_macro2::Span;

/// Validates all placeholders in steps reference columns in the Examples table.
///
/// Checks step text, docstrings, and data table cells for placeholder references.
/// Returns an error on the first undefined placeholder found.
///
/// # Arguments
///
/// * `steps` - The parsed steps from the scenario
/// * `headers` - Column headers from the Examples table
///
/// # Returns
///
/// `Ok(())` if all placeholders are valid, or `Err` with a descriptive error
/// message if any placeholder references a non-existent column.
pub fn validate_step_placeholders(
    steps: &[ParsedStep],
    headers: &[String],
) -> Result<(), syn::Error> {
    for step in steps {
        // Validate step text
        validate_text_placeholders(&step.text, headers, "step")?;

        // Validate docstring if present
        if let Some(ref docstring) = step.docstring {
            validate_text_placeholders(docstring, headers, "docstring")?;
        }

        // Validate data table cells if present
        if let Some(ref table) = step.table {
            for row in table {
                for cell in row {
                    validate_text_placeholders(cell, headers, "table cell")?;
                }
            }
        }
    }
    Ok(())
}

/// Validates placeholders in a single text string.
fn validate_text_placeholders(
    text: &str,
    headers: &[String],
    context: &str,
) -> Result<(), syn::Error> {
    for cap in PLACEHOLDER_RE.captures_iter(text) {
        let placeholder = &cap[1];
        if !headers.iter().any(|h| h == placeholder) {
            return Err(syn::Error::new(
                Span::call_site(),
                format!(
                    "Placeholder '<{placeholder}>' in {context} not found in Examples table. \
                     Available columns: [{}]",
                    headers.join(", ")
                ),
            ));
        }
    }
    Ok(())
}

#[cfg(test)]
#[expect(clippy::unwrap_used, reason = "tests use unwrap for brevity")]
mod tests {
    use super::*;
    use crate::StepKeyword;

    fn make_step(text: &str) -> ParsedStep {
        ParsedStep {
            keyword: StepKeyword::Given,
            text: text.to_string(),
            docstring: None,
            table: None,
            #[cfg(feature = "compile-time-validation")]
            span: Span::call_site(),
        }
    }

    fn make_step_with_docstring(text: &str, docstring: &str) -> ParsedStep {
        ParsedStep {
            keyword: StepKeyword::Given,
            text: text.to_string(),
            docstring: Some(docstring.to_string()),
            table: None,
            #[cfg(feature = "compile-time-validation")]
            span: Span::call_site(),
        }
    }

    fn make_step_with_table(text: &str, table: Vec<Vec<String>>) -> ParsedStep {
        ParsedStep {
            keyword: StepKeyword::Given,
            text: text.to_string(),
            docstring: None,
            table: Some(table),
            #[cfg(feature = "compile-time-validation")]
            span: Span::call_site(),
        }
    }

    #[test]
    fn valid_placeholder_in_step_text() {
        let steps = vec![make_step("I have <count> items")];
        let headers = vec!["count".to_string()];

        let result = validate_step_placeholders(&steps, &headers);
        assert!(result.is_ok());
    }

    #[test]
    fn valid_multiple_placeholders() {
        let steps = vec![make_step("I have <count> <item>")];
        let headers = vec!["count".to_string(), "item".to_string()];

        let result = validate_step_placeholders(&steps, &headers);
        assert!(result.is_ok());
    }

    #[test]
    fn invalid_placeholder_in_step_text() {
        let steps = vec![make_step("I have <undefined> items")];
        let headers = vec!["count".to_string()];

        let result = validate_step_placeholders(&steps, &headers);
        assert!(result.is_err());

        let err = result.unwrap_err();
        let msg = err.to_string();
        assert!(msg.contains("<undefined>"));
        assert!(msg.contains("step"));
        assert!(msg.contains("count"));
    }

    #[test]
    fn valid_placeholder_in_docstring() {
        let steps = vec![make_step_with_docstring(
            "step text",
            "docstring with <value>",
        )];
        let headers = vec!["value".to_string()];

        let result = validate_step_placeholders(&steps, &headers);
        assert!(result.is_ok());
    }

    #[test]
    fn invalid_placeholder_in_docstring() {
        let steps = vec![make_step_with_docstring(
            "step text",
            "docstring with <undefined>",
        )];
        let headers = vec!["value".to_string()];

        let result = validate_step_placeholders(&steps, &headers);
        assert!(result.is_err());

        let err = result.unwrap_err();
        let msg = err.to_string();
        assert!(msg.contains("<undefined>"));
        assert!(msg.contains("docstring"));
    }

    #[test]
    fn valid_placeholder_in_table() {
        let steps = vec![make_step_with_table(
            "step text",
            vec![vec!["<value>".to_string(), "static".to_string()]],
        )];
        let headers = vec!["value".to_string()];

        let result = validate_step_placeholders(&steps, &headers);
        assert!(result.is_ok());
    }

    #[test]
    fn invalid_placeholder_in_table() {
        let steps = vec![make_step_with_table(
            "step text",
            vec![vec!["<undefined>".to_string()]],
        )];
        let headers = vec!["value".to_string()];

        let result = validate_step_placeholders(&steps, &headers);
        assert!(result.is_err());

        let err = result.unwrap_err();
        let msg = err.to_string();
        assert!(msg.contains("<undefined>"));
        assert!(msg.contains("table cell"));
    }

    #[test]
    fn no_placeholders_is_valid() {
        let steps = vec![make_step("I have 5 items")];
        let headers = vec!["count".to_string()];

        let result = validate_step_placeholders(&steps, &headers);
        assert!(result.is_ok());
    }

    #[test]
    fn empty_steps_is_valid() {
        let steps: Vec<ParsedStep> = vec![];
        let headers = vec!["count".to_string()];

        let result = validate_step_placeholders(&steps, &headers);
        assert!(result.is_ok());
    }
}
