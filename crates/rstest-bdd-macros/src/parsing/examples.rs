//! Extraction of example tables from scenarios.

use proc_macro2::TokenStream;

use crate::{
    utils::errors::error_to_tokens,
    validation::examples::{
        extract_and_validate_headers,
        flatten_and_validate_rows,
        validate_header_consistency,
    },
};

/// Rows parsed from a `Scenario Outline` examples table.
///
/// The `row_tags` collection mirrors `rows` one-to-one: each row inherits the
/// union of feature, scenario, and examples tags at the corresponding index.
/// This invariant is encoded as `row_tags.len() == rows.len()` and enforced by
/// the parser when constructing the table.
#[derive(Clone, Debug)]
pub(crate) struct ExampleTable {
    /// Stores the internal `headers` value.
    pub(crate) headers: Vec<String>,
    /// Stores the internal `rows` value.
    pub(crate) rows: Vec<Vec<String>>,
    /// Union of feature, scenario, and examples tags for each row.
    /// Guaranteed to match `rows` in length so callers can zip the sequences
    /// safely without additional bounds checks.
    pub(crate) row_tags: Vec<Vec<String>>,
}

/// Provides the internal `should_process_outline` operation.
fn should_process_outline(scenario: &gherkin::Scenario) -> bool {
    scenario.keyword == "Scenario Outline" || !scenario.examples.is_empty()
}

/// Provides the internal `get_first_examples_table` operation.
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

/// Build one merged tag set per data row, in row order across Examples blocks.
///
/// Each Examples block contributes its own tags merged with `base_tags` for
/// every row after its header; blocks without a table contribute nothing.
fn collect_row_tags(
    scenario: &gherkin::Scenario,
    base_tags: &[String],
    row_count: usize,
) -> Vec<Vec<String>> {
    let mut row_tags = Vec::with_capacity(row_count);
    for (table, ex_tags) in scenario
        .examples
        .iter()
        .filter_map(|ex| ex.table.as_ref().map(|table| (table, &ex.tags)))
    {
        let examples_tags = crate::parsing::tags::merge_tag_sets(base_tags, ex_tags);
        let data_rows = table.rows.len().saturating_sub(1);
        row_tags.extend(std::iter::repeat_n(examples_tags, data_rows));
    }
    row_tags
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

    let row_tags = collect_row_tags(scenario, base_tags, rows.len());

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

    use gherkin::{LineCol, Scenario, Span};

    use super::get_first_examples_table;

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
            name: name.to_owned(),
            ..empty_scenario()
        }
    }

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
