//! Validates placeholder references in Scenario Outline steps.
//!
//! This module ensures that all `<placeholder>` tokens in step text, docstrings,
//! and data tables reference columns that exist in the Examples table. Validation
//! happens at compile time during macro expansion, providing early feedback for
//! misconfigured scenario outlines.

use crate::parsing::feature::ParsedStep;
use crate::parsing::placeholder::PLACEHOLDER_RE;
use proc_macro2::Span;

/// Location where placeholder validation is performed.
#[derive(Debug, Clone, Copy)]
pub(crate) enum ValidationContext {
    /// Validation in step text
    Step,
    /// Validation in step docstring
    Docstring,
    /// Validation in data table cell
    TableCell,
}

impl ValidationContext {
    fn as_str(self) -> &'static str {
        match self {
            Self::Step => "step",
            Self::Docstring => "docstring",
            Self::TableCell => "table cell",
        }
    }
}

/// Column headers from an Examples table.
#[derive(Debug, Clone, Copy)]
pub(crate) struct ExampleHeaders<'a>(&'a [String]);

impl<'a> ExampleHeaders<'a> {
    pub fn new(headers: &'a [String]) -> Self {
        Self(headers)
    }

    pub fn contains(&self, name: &str) -> bool {
        self.0.iter().any(|h| h == name)
    }

    pub fn join(&self, sep: &str) -> String {
        self.0.join(sep)
    }
}

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
    headers: ExampleHeaders<'_>,
) -> Result<(), syn::Error> {
    for step in steps {
        validate_step_text(&step.text, headers)?;
        validate_step_docstring(step.docstring.as_ref(), headers)?;
        validate_step_table(step.table.as_ref(), headers)?;
    }
    Ok(())
}

/// Validates placeholders in step text.
fn validate_step_text(text: &str, headers: ExampleHeaders<'_>) -> Result<(), syn::Error> {
    validate_text_placeholders(text, headers, ValidationContext::Step)
}

/// Validates placeholders in step docstring if present.
fn validate_step_docstring(
    docstring: Option<&String>,
    headers: ExampleHeaders<'_>,
) -> Result<(), syn::Error> {
    if let Some(docstring) = docstring {
        validate_text_placeholders(docstring, headers, ValidationContext::Docstring)?;
    }
    Ok(())
}

/// Validates placeholders in step data table if present.
fn validate_step_table(
    table: Option<&Vec<Vec<String>>>,
    headers: ExampleHeaders<'_>,
) -> Result<(), syn::Error> {
    if let Some(table) = table {
        for row in table {
            for cell in row {
                validate_text_placeholders(cell, headers, ValidationContext::TableCell)?;
            }
        }
    }
    Ok(())
}

/// Validates placeholders in a single text string.
fn validate_text_placeholders(
    text: &str,
    headers: ExampleHeaders<'_>,
    context: ValidationContext,
) -> Result<(), syn::Error> {
    for cap in PLACEHOLDER_RE.captures_iter(text) {
        let placeholder = &cap[1];
        if !headers.contains(placeholder) {
            return Err(syn::Error::new(
                Span::call_site(),
                format!(
                    "Placeholder '<{placeholder}>' in {} not found in Examples table. \
                     Available columns: [{}]",
                    context.as_str(),
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
    use rstest::rstest;

    /// Builder for creating test `ParsedStep` instances.
    struct ParsedStepBuilder {
        text: String,
        docstring: Option<String>,
        table: Option<Vec<Vec<String>>>,
    }

    fn make_step_builder() -> ParsedStepBuilder {
        ParsedStepBuilder {
            text: String::new(),
            docstring: None,
            table: None,
        }
    }

    impl ParsedStepBuilder {
        fn with_text(mut self, text: &str) -> Self {
            self.text = text.to_string();
            self
        }

        fn with_docstring(mut self, docstring: Option<&str>) -> Self {
            self.docstring = docstring.map(ToString::to_string);
            self
        }

        fn with_table(mut self, table: Option<Vec<Vec<String>>>) -> Self {
            self.table = table;
            self
        }

        fn build(self) -> ParsedStep {
            ParsedStep {
                keyword: StepKeyword::Given,
                text: self.text,
                docstring: self.docstring,
                table: self.table,
                #[cfg(feature = "compile-time-validation")]
                span: Span::call_site(),
            }
        }
    }

    /// Asserts that placeholder validation fails with expected error content.
    fn assert_placeholder_error(
        steps: &[ParsedStep],
        headers: ExampleHeaders<'_>,
        expected_placeholder: &str,
        expected_context: ValidationContext,
    ) {
        let result = validate_step_placeholders(steps, headers);
        assert!(result.is_err());

        let err = result.unwrap_err();
        let msg = err.to_string();
        assert!(
            msg.contains(expected_placeholder),
            "Error should contain placeholder '{expected_placeholder}': {msg}"
        );
        assert!(
            msg.contains(expected_context.as_str()),
            "Error should contain context '{}': {msg}",
            expected_context.as_str()
        );
    }

    /// Asserts that placeholder validation succeeds.
    fn assert_valid_placeholders(steps: &[ParsedStep], headers: &[String]) {
        let result = validate_step_placeholders(steps, ExampleHeaders::new(headers));
        assert!(
            result.is_ok(),
            "Expected validation to pass but got error: {:?}",
            result.unwrap_err()
        );
    }

    #[rstest]
    #[case::step_text("I have <count> items", vec!["count"], None, None)]
    #[case::multiple_placeholders("I have <count> <item>", vec!["count", "item"], None, None)]
    #[case::docstring("step text", vec!["value"], Some("docstring with <value>"), None)]
    #[case::table("step text", vec!["value"], None, Some(vec![vec!["<value>".to_string(), "static".to_string()]]))]
    #[case::no_placeholders("I have 5 items", vec!["count"], None, None)]
    fn valid_placeholder_tests(
        #[case] text: &str,
        #[case] header_strs: Vec<&str>,
        #[case] docstring: Option<&str>,
        #[case] table: Option<Vec<Vec<String>>>,
    ) {
        let steps = vec![
            make_step_builder()
                .with_text(text)
                .with_docstring(docstring)
                .with_table(table)
                .build(),
        ];
        let headers: Vec<String> = header_strs.into_iter().map(ToString::to_string).collect();

        assert_valid_placeholders(&steps, &headers);
    }

    #[rstest]
    #[case::step_text("I have <undefined> items", vec!["count"], None, None, "<undefined>", ValidationContext::Step)]
    #[case::docstring("step text", vec!["value"], Some("docstring with <undefined>"), None, "<undefined>", ValidationContext::Docstring)]
    #[case::table("step text", vec!["value"], None, Some(vec![vec!["<undefined>".to_string()]]), "<undefined>", ValidationContext::TableCell)]
    fn invalid_placeholder_tests(
        #[case] text: &str,
        #[case] header_strs: Vec<&str>,
        #[case] docstring: Option<&str>,
        #[case] table: Option<Vec<Vec<String>>>,
        #[case] expected_placeholder: &str,
        #[case] expected_context: ValidationContext,
    ) {
        let steps = vec![
            make_step_builder()
                .with_text(text)
                .with_docstring(docstring)
                .with_table(table)
                .build(),
        ];
        let headers: Vec<String> = header_strs.into_iter().map(ToString::to_string).collect();

        assert_placeholder_error(
            &steps,
            ExampleHeaders::new(&headers),
            expected_placeholder,
            expected_context,
        );
    }

    #[test]
    fn empty_steps_is_valid() {
        let steps: Vec<ParsedStep> = vec![];
        let headers = vec!["count".to_string()];

        assert_valid_placeholders(&steps, &headers);
    }
}
