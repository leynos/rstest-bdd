//! Extraction of example tables from scenarios.

use crate::utils::errors::error_to_tokens;
use crate::validation::examples::{
    extract_and_validate_headers, flatten_and_validate_rows, validate_header_consistency,
};

/// Rows parsed from a `Scenario Outline` examples table.
#[derive(Clone)]
pub(crate) struct ExampleTable {
    pub(crate) headers: Vec<String>,
    pub(crate) rows: Vec<Vec<String>>,
}

fn should_process_outline(scenario: &gherkin::Scenario) -> bool {
    scenario.keyword == "Scenario Outline" || !scenario.examples.is_empty()
}

fn get_first_examples_table(
    scenario: &gherkin::Scenario,
) -> Result<&gherkin::Table, proc_macro::TokenStream> {
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
) -> Result<Option<ExampleTable>, proc_macro::TokenStream> {
    if !should_process_outline(scenario) {
        return Ok(None);
    }

    let first_table = get_first_examples_table(scenario)?;
    let headers = extract_and_validate_headers(first_table)?;
    validate_header_consistency(scenario, &headers)?;
    let rows = flatten_and_validate_rows(scenario, headers.len())?;

    Ok(Some(ExampleTable { headers, rows }))
}
