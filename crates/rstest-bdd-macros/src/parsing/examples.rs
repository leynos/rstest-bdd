//! Extraction of example tables from scenarios.

use crate::utils::errors::error_to_tokens;
use crate::validation::examples::{
    extract_and_validate_headers, flatten_and_validate_rows, validate_header_consistency,
};
use proc_macro2::TokenStream;

/// Rows parsed from a `Scenario Outline` examples table.
///
/// The `row_tags` collection mirrors `rows` one-to-one: each row inherits the
/// union of feature, scenario, and examples tags at the corresponding index.
/// This invariant is encoded as `row_tags.len() == rows.len()` and enforced by
/// the parser when constructing the table.
#[derive(Clone, Debug)]
pub(crate) struct ExampleTable {
    pub(crate) headers: Vec<String>,
    pub(crate) rows: Vec<Vec<String>>,
    /// Union of feature, scenario, and examples tags for each row.
    /// Guaranteed to match `rows` in length so callers can zip the sequences
    /// safely without additional bounds checks.
    pub(crate) row_tags: Vec<Vec<String>>,
}

fn should_process_outline(scenario: &gherkin::Scenario) -> bool {
    scenario.keyword == "Scenario Outline" || !scenario.examples.is_empty()
}

fn get_first_examples_table(scenario: &gherkin::Scenario) -> Result<&gherkin::Table, TokenStream> {
    scenario
        .examples
        .first()
        .and_then(|ex| ex.table.as_ref())
        .ok_or_else(|| {
            error_to_tokens(&syn::Error::new(
                proc_macro2::Span::call_site(),
                format!(
                    "Scenario Outline missing Examples table for '{}'",
                    scenario.name
                ),
            ))
        })
}

/// Extract examples table data from a scenario if present.
pub(crate) fn extract_examples(
    scenario: &gherkin::Scenario,
    base_tags: &[String],
) -> Result<Option<ExampleTable>, TokenStream> {
    if !should_process_outline(scenario) {
        return Ok(None);
    }

    let first_table = get_first_examples_table(scenario)?;
    let headers = extract_and_validate_headers(first_table)?;
    validate_header_consistency(scenario, &headers)?;
    let rows = flatten_and_validate_rows(scenario, headers.len())?;

    let mut row_tags = Vec::with_capacity(rows.len());
    for ex in &scenario.examples {
        let Some(table) = ex.table.as_ref() else {
            continue;
        };
        let examples_tags = crate::parsing::tags::merge_tag_sets(base_tags, &ex.tags);
        for _ in table.rows.iter().skip(1) {
            row_tags.push(examples_tags.clone());
        }
    }

    debug_assert_eq!(
        row_tags.len(),
        rows.len(),
        "examples row tags must align with extracted rows",
    );

    Ok(Some(ExampleTable {
        headers,
        rows,
        row_tags,
    }))
}

#[cfg(test)]
mod tests {
    //! Tests for example table extraction.

    use super::get_first_examples_table;
    use gherkin::{LineCol, Scenario, Span};

    fn empty_scenario() -> Scenario {
        Scenario {
            keyword: String::new(),
            name: String::new(),
            description: None,
            steps: Vec::new(),
            examples: Vec::new(),
            tags: Vec::new(),
            span: Span { start: 0, end: 0 },
            position: LineCol { line: 0, col: 0 },
        }
    }

    fn scenario_outline_without_examples(name: &str) -> Scenario {
        Scenario {
            keyword: "Scenario Outline".into(),
            name: name.to_string(),
            ..empty_scenario()
        }
    }

    #[expect(
        clippy::expect_used,
        reason = "tests assert specific error paths; panics aid debugging"
    )]
    #[test]
    fn missing_examples_error_includes_scenario_name() {
        let scenario = scenario_outline_without_examples("outline without examples");

        let tokens =
            get_first_examples_table(&scenario).expect_err("expected missing examples error");

        let message = tokens.to_string();
        assert!(
            message.contains("Scenario Outline missing Examples table"),
            "error message should mention missing examples; got: {message}",
        );
        assert!(
            message.contains(&scenario.name),
            "error message should include scenario name; got: {message}",
        );
    }
}
