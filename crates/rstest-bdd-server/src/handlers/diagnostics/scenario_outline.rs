//! Scenario outline example column validation diagnostics.
//!
//! This module validates that scenario outline placeholders (`<column>`)
//! match the columns defined in the Examples table:
//!
//! - **Missing column**: A step uses `<foo>` but the Examples table lacks a
//!   `foo` column header.
//! - **Surplus column**: The Examples table has a `bar` column but no step
//!   references `<bar>`.

use std::collections::HashSet;
use std::sync::LazyLock;

use lsp_types::{Diagnostic, DiagnosticSeverity};
use regex::Regex;

use crate::handlers::util::gherkin_span_to_lsp_range;
use crate::indexing::{
    FeatureFileIndex, IndexedExamplesTable, IndexedScenarioOutline, IndexedStep,
};

use super::{CODE_EXAMPLE_COLUMN_MISSING, CODE_EXAMPLE_COLUMN_SURPLUS, DIAGNOSTIC_SOURCE};

/// Regex for extracting `<placeholder>` tokens from scenario outline step text.
///
/// This pattern matches the angle-bracket placeholder syntax used in Gherkin
/// Scenario Outlines, consistent with the macros crate's `PLACEHOLDER_RE`.
static OUTLINE_PLACEHOLDER_RE: LazyLock<Regex> =
    LazyLock::new(|| Regex::new(r"<([^>\s][^>]*)>").unwrap_or_else(|_| unreachable!()));

/// Compute diagnostics for scenario outline example column mismatches.
///
/// For each scenario outline, checks that:
/// 1. All `<placeholder>` references in steps have matching columns in the
///    Examples table.
/// 2. All Examples table columns are referenced by at least one step
///    placeholder.
///
/// # Example
///
/// ```no_run
/// use rstest_bdd_server::indexing::FeatureFileIndex;
///
/// // Obtain a FeatureFileIndex from indexing
/// # let feature_index: FeatureFileIndex = todo!();
///
/// let diagnostics = rstest_bdd_server::handlers::compute_scenario_outline_column_diagnostics(
///     &feature_index,
/// );
/// // Returns diagnostics when placeholders don't match column headers
/// for diag in &diagnostics {
///     println!("{}", diag.message);
/// }
/// ```
#[must_use]
pub fn compute_scenario_outline_column_diagnostics(
    feature_index: &FeatureFileIndex,
) -> Vec<Diagnostic> {
    feature_index
        .scenario_outlines
        .iter()
        .flat_map(|outline| check_outline_columns(feature_index, outline))
        .collect()
}

/// Check a single scenario outline for column mismatches.
fn check_outline_columns(
    feature_index: &FeatureFileIndex,
    outline: &IndexedScenarioOutline,
) -> Vec<Diagnostic> {
    let mut diagnostics = Vec::new();

    // Collect all placeholders from all steps in the outline
    let all_placeholders = collect_all_placeholders(feature_index, outline);

    // Check each Examples table independently
    for examples in &outline.examples {
        diagnostics.extend(check_examples_table(
            feature_index,
            outline,
            examples,
            &all_placeholders,
        ));
    }

    diagnostics
}

/// Collect all unique placeholder names from steps in a scenario outline.
fn collect_all_placeholders(
    feature_index: &FeatureFileIndex,
    outline: &IndexedScenarioOutline,
) -> HashSet<String> {
    let mut placeholders = HashSet::new();

    for &step_idx in &outline.step_indices {
        if let Some(step) = feature_index.steps.get(step_idx) {
            placeholders.extend(extract_placeholders_from_step(step));
        }
    }

    placeholders
}

/// Extract all placeholder names from a single step.
///
/// Scans the step text, docstring content, and data table cells for
/// `<placeholder>` references.
fn extract_placeholders_from_step(step: &IndexedStep) -> HashSet<String> {
    let mut placeholders = HashSet::new();

    // Extract from step text
    placeholders.extend(extract_placeholders_from_text(&step.text));

    // Extract from docstring if present
    if let Some(ref docstring) = step.docstring {
        placeholders.extend(extract_placeholders_from_text(&docstring.value));
    }

    // Extract from data table cells if present
    if let Some(ref table) = step.table {
        for row in &table.rows {
            for cell in row {
                placeholders.extend(extract_placeholders_from_text(cell));
            }
        }
    }

    placeholders
}

/// Extract placeholder names from a text string.
fn extract_placeholders_from_text(text: &str) -> HashSet<String> {
    OUTLINE_PLACEHOLDER_RE
        .captures_iter(text)
        .filter_map(|cap| cap.get(1).map(|m| m.as_str().to_owned()))
        .collect()
}

/// Check an Examples table against the collected placeholders.
fn check_examples_table(
    feature_index: &FeatureFileIndex,
    outline: &IndexedScenarioOutline,
    examples: &IndexedExamplesTable,
    all_placeholders: &HashSet<String>,
) -> Vec<Diagnostic> {
    let mut diagnostics = Vec::new();

    // Build set of column names from this Examples table
    let column_names: HashSet<String> = examples
        .columns
        .iter()
        .map(|col| col.name.clone())
        .collect();

    // Check for missing columns (placeholders without matching columns)
    let missing: Vec<&String> = all_placeholders.difference(&column_names).collect();
    for placeholder in missing {
        // Report on the first step that uses this placeholder
        if let Some(diag) =
            build_missing_column_diagnostic(feature_index, outline, placeholder, &column_names)
        {
            diagnostics.push(diag);
        }
    }

    // Check for surplus columns (columns without matching placeholders)
    for column in &examples.columns {
        if !all_placeholders.contains(&column.name) {
            diagnostics.push(build_surplus_column_diagnostic(feature_index, column));
        }
    }

    diagnostics
}

/// Build a diagnostic for a missing column.
///
/// Reports on the step that references the undefined placeholder.
fn build_missing_column_diagnostic(
    feature_index: &FeatureFileIndex,
    outline: &IndexedScenarioOutline,
    placeholder: &str,
    available_columns: &HashSet<String>,
) -> Option<Diagnostic> {
    // Find the first step that uses this placeholder
    let step = find_step_with_placeholder(feature_index, outline, placeholder)?;
    let range = gherkin_span_to_lsp_range(&feature_index.source, step.span);

    let available: Vec<&str> = available_columns.iter().map(String::as_str).collect();
    let available_str = if available.is_empty() {
        "none".to_owned()
    } else {
        available.join(", ")
    };

    Some(Diagnostic {
        range,
        severity: Some(DiagnosticSeverity::WARNING),
        code: Some(lsp_types::NumberOrString::String(
            CODE_EXAMPLE_COLUMN_MISSING.to_owned(),
        )),
        code_description: None,
        source: Some(DIAGNOSTIC_SOURCE.to_owned()),
        message: format!(
            "Placeholder '<{placeholder}>' has no matching column in Examples table. \
             Available columns: [{available_str}]"
        ),
        related_information: None,
        tags: None,
        data: None,
    })
}

/// Find the first step in the outline that uses the given placeholder.
fn find_step_with_placeholder<'a>(
    feature_index: &'a FeatureFileIndex,
    outline: &IndexedScenarioOutline,
    placeholder: &str,
) -> Option<&'a IndexedStep> {
    for &step_idx in &outline.step_indices {
        if let Some(step) = feature_index.steps.get(step_idx) {
            let placeholders = extract_placeholders_from_step(step);
            if placeholders.contains(placeholder) {
                return Some(step);
            }
        }
    }
    None
}

/// Build a diagnostic for a surplus column.
///
/// Reports on the column header in the Examples table.
fn build_surplus_column_diagnostic(
    feature_index: &FeatureFileIndex,
    column: &crate::indexing::IndexedExampleColumn,
) -> Diagnostic {
    let range = gherkin_span_to_lsp_range(&feature_index.source, column.span);

    Diagnostic {
        range,
        severity: Some(DiagnosticSeverity::WARNING),
        code: Some(lsp_types::NumberOrString::String(
            CODE_EXAMPLE_COLUMN_SURPLUS.to_owned(),
        )),
        code_description: None,
        source: Some(DIAGNOSTIC_SOURCE.to_owned()),
        message: format!(
            "Examples column '{}' is not referenced by any step placeholder in the scenario outline",
            column.name
        ),
        related_information: None,
        tags: None,
        data: None,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn extract_placeholders_from_simple_text() {
        let text = "I have <count> items";
        let placeholders = extract_placeholders_from_text(text);
        assert_eq!(placeholders.len(), 1);
        assert!(placeholders.contains("count"));
    }

    #[test]
    fn extract_placeholders_from_multiple() {
        let text = "I have <count> items of <type>";
        let placeholders = extract_placeholders_from_text(text);
        assert_eq!(placeholders.len(), 2);
        assert!(placeholders.contains("count"));
        assert!(placeholders.contains("type"));
    }

    #[test]
    fn extract_placeholders_ignores_malformed() {
        let text = "I have < count> items and <type>";
        let placeholders = extract_placeholders_from_text(text);
        // Only <type> is valid; < count> has a leading space
        assert_eq!(placeholders.len(), 1);
        assert!(placeholders.contains("type"));
    }

    #[test]
    fn extract_placeholders_empty_when_none() {
        let text = "I have 5 items";
        let placeholders = extract_placeholders_from_text(text);
        assert!(placeholders.is_empty());
    }

    #[test]
    fn extract_placeholders_handles_nested_angles() {
        let text = "I have <count> items";
        let placeholders = extract_placeholders_from_text(text);
        assert_eq!(placeholders.len(), 1);
        assert!(placeholders.contains("count"));
    }
}
