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
/// The parser enforces this invariant when constructing the table.
#[derive(Clone)]
pub(crate) struct ExampleTable {
    pub(crate) headers: Vec<String>,
    pub(crate) rows: Vec<Vec<String>>,
    /// Union of feature, scenario, and examples tags for each row.
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
                "Scenario Outline missing Examples table",
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
