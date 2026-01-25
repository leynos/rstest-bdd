//! Scenario outline indexing helpers.
//!
//! This module contains functions for building `IndexedScenarioOutline` and
//! `IndexedExamplesTable` structures from parsed Gherkin AST nodes.

use gherkin::Span;

use super::FeatureSource;
use super::table::extract_header_cell_spans;
use crate::indexing::{IndexedExampleColumn, IndexedExamplesTable, IndexedScenarioOutline};

/// Check if a scenario is a Scenario Outline (or Scenario Template).
///
/// A scenario is considered an outline if it has any examples, regardless of
/// the keyword used.
pub(super) fn is_scenario_outline(scenario: &gherkin::Scenario) -> bool {
    !scenario.examples.is_empty()
}

/// Build an `IndexedScenarioOutline` from a parsed scenario.
pub(super) fn build_scenario_outline(
    source: FeatureSource<'_>,
    scenario: &gherkin::Scenario,
    step_start_index: usize,
    step_end_index: usize,
    background_step_indices: Vec<usize>,
) -> IndexedScenarioOutline {
    let step_indices: Vec<usize> = (step_start_index..step_end_index).collect();
    let examples = build_examples_tables(source, &scenario.examples);

    IndexedScenarioOutline {
        name: scenario.name.clone(),
        span: scenario.span,
        step_indices,
        background_step_indices,
        examples,
    }
}

/// Build `IndexedExamplesTable` entries from parsed Examples blocks.
fn build_examples_tables(
    source: FeatureSource<'_>,
    examples: &[gherkin::Examples],
) -> Vec<IndexedExamplesTable> {
    let mut tables = Vec::with_capacity(examples.len());

    for ex in examples {
        let Some(table) = ex.table.as_ref() else {
            continue;
        };
        let columns = extract_columns_for_table(source, table.span, &table.rows);

        // Skip tables with empty columns (e.g., unresolved header spans or
        // length mismatches) to avoid misleading diagnostics.
        if columns.is_empty() {
            continue;
        }

        tables.push(IndexedExamplesTable {
            span: ex.span,
            columns,
        });
    }

    tables
}

/// Extract column headers with spans from an Examples table.
fn extract_columns_for_table(
    source: FeatureSource<'_>,
    table_span: Span,
    rows: &[Vec<String>],
) -> Vec<IndexedExampleColumn> {
    let Some(header_spans) = extract_header_cell_spans(source, table_span) else {
        return Vec::new();
    };
    let Some(header_row) = rows.first() else {
        return Vec::new();
    };
    build_indexed_example_columns(header_row, header_spans)
}

/// Build indexed example columns from a header row and corresponding spans.
///
/// Returns an empty vector if the header row and spans have different lengths.
fn build_indexed_example_columns(
    header_row: &[String],
    header_spans: Vec<Span>,
) -> Vec<IndexedExampleColumn> {
    if header_row.len() != header_spans.len() {
        return Vec::new();
    }
    header_row
        .iter()
        .cloned()
        .zip(header_spans)
        .map(|(name, span)| IndexedExampleColumn { name, span })
        .collect()
}

/// Extract all example columns from a parsed feature.
pub(super) fn extract_example_columns(
    source: FeatureSource<'_>,
    feature: &gherkin::Feature,
) -> Vec<IndexedExampleColumn> {
    let mut columns = Vec::new();
    for scenario in &feature.scenarios {
        collect_example_columns_for_scenario(source, &scenario.examples, &mut columns);
    }
    for rule in &feature.rules {
        for scenario in &rule.scenarios {
            collect_example_columns_for_scenario(source, &scenario.examples, &mut columns);
        }
    }
    columns
}

/// Collect example columns from a list of Examples blocks.
fn collect_example_columns_for_scenario(
    source: FeatureSource<'_>,
    examples: &[gherkin::Examples],
    columns: &mut Vec<IndexedExampleColumn>,
) {
    for ex in examples {
        let Some(table) = ex.table.as_ref() else {
            continue;
        };
        let Some(header_spans) = extract_header_cell_spans(source, table.span) else {
            continue;
        };
        let Some(header_row) = table.rows.first() else {
            continue;
        };
        columns.extend(build_indexed_example_columns(header_row, header_spans));
    }
}
