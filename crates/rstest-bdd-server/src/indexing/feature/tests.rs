//! Tests for feature file indexing.

use tempfile::TempDir;

use super::*;
use crate::indexing::IndexedExampleColumn;

#[test]
fn indexes_steps_tables_docstrings_and_example_columns() {
    let dir = TempDir::new().expect("temp dir");
    let path = dir.path().join("demo.feature");

    let feature = concat!(
        "Feature: demo\n",
        "  Scenario Outline: outline\n",
        "    Given a message\n",
        "      \"\"\"\n",
        "      hello\n",
        "      \"\"\"\n",
        "    When numbers\n",
        "      | a | b |\n",
        "      | 1 | 2 |\n",
        "    Then result is <Result>\n",
        "    Examples:\n",
        "      | Result | Extra |\n",
        "      | ok     | x     |\n",
    );

    std::fs::write(&path, feature).expect("write feature file");

    let index = index_feature_file(&path).expect("index feature file");
    assert_eq!(index.steps.len(), 3, "expected three indexed steps");

    assert_example_column_names(&index.example_columns, ["Result", "Extra"]);
    assert_docstring_step(&index);
    assert_table_step(&index);
    assert_scenario_outline(&index);
}

/// Assert the feature-level example column names, in order.
fn assert_example_column_names(columns: &[IndexedExampleColumn], expected: [&str; 2]) {
    let names: Vec<&str> = columns.iter().map(|column| column.name.as_str()).collect();
    assert_eq!(names, expected, "unexpected example column names");
}

/// Assert the first step carries a docstring with a non-empty span.
fn assert_docstring_step(index: &FeatureFileIndex) {
    let Some(given) = index.steps.first() else {
        panic!("expected indexed steps");
    };
    assert_eq!(given.keyword.trim(), "Given", "unexpected first keyword");
    let Some(doc) = given.docstring.as_ref() else {
        panic!("expected the first step to carry a doc string");
    };
    assert!(
        doc.span.start < doc.span.end,
        "docstring span should be non-empty"
    );
}

/// Assert the second step carries the parsed data table.
fn assert_table_step(index: &FeatureFileIndex) {
    let Some(when) = index.steps.get(1) else {
        panic!("expected a second indexed step");
    };
    assert_eq!(when.keyword.trim(), "When", "unexpected second keyword");
    let Some(table) = when.table.as_ref() else {
        panic!("expected the second step to carry a data table");
    };
    let Some(first_row) = table.rows.first() else {
        panic!("expected the data table to have rows");
    };
    assert_eq!(first_row, &vec!["a".to_owned(), "b".to_owned()]);
    assert!(
        table.span.start < table.span.end,
        "table span should be non-empty"
    );
}

/// Assert the scenario outline and its single Examples table.
fn assert_scenario_outline(index: &FeatureFileIndex) {
    assert_eq!(index.scenario_outlines.len(), 1, "expected one outline");
    let Some(outline) = index.scenario_outlines.first() else {
        panic!("expected a scenario outline");
    };
    assert_eq!(outline.name, "outline", "unexpected outline name");
    assert_eq!(
        outline.step_indices,
        vec![0, 1, 2],
        "unexpected step indices"
    );

    let Some(examples_table) = outline.examples.first() else {
        panic!("expected an examples table");
    };
    assert_example_column_names(&examples_table.columns, ["Result", "Extra"]);
}

#[test]
fn indexes_multiple_examples_tables() {
    let dir = TempDir::new().expect("temp dir");
    let path = dir.path().join("multi.feature");

    let feature = concat!(
        "Feature: multi\n",
        "  Scenario Outline: outline\n",
        "    Given I have <count> items\n",
        "    Examples: first\n",
        "      | count |\n",
        "      | 1     |\n",
        "    Examples: second\n",
        "      | count | extra |\n",
        "      | 2     | x     |\n",
    );

    std::fs::write(&path, feature).expect("write feature file");

    let index = index_feature_file(&path).expect("index feature file");
    assert_eq!(index.scenario_outlines.len(), 1);

    let outline = index
        .scenario_outlines
        .first()
        .expect("expected scenario outline");
    assert_eq!(outline.examples.len(), 2);

    let first_table = outline.examples.first().expect("first table");
    assert_eq!(first_table.columns.len(), 1);
    let first_col = first_table.columns.first().expect("first column");
    assert_eq!(first_col.name, "count");

    let second_table = outline.examples.get(1).expect("second table");
    assert_eq!(second_table.columns.len(), 2);
    let col0 = second_table.columns.first().expect("first col");
    let col1 = second_table.columns.get(1).expect("second col");
    assert_eq!(col0.name, "count");
    assert_eq!(col1.name, "extra");
}

#[test]
fn regular_scenario_not_indexed_as_outline() {
    let dir = TempDir::new().expect("temp dir");
    let path = dir.path().join("regular.feature");

    let feature = concat!(
        "Feature: regular\n",
        "  Scenario: not an outline\n",
        "    Given a step\n",
    );

    std::fs::write(&path, feature).expect("write feature file");

    let index = index_feature_file(&path).expect("index feature file");
    assert_eq!(index.steps.len(), 1);
    assert!(
        index.scenario_outlines.is_empty(),
        "regular scenarios should not be indexed as outlines"
    );
}

#[test]
fn docstring_span_includes_backtick_delimiters() {
    let dir = TempDir::new().expect("temp dir");
    let path = dir.path().join("ticks.feature");
    let feature = concat!(
        "Feature: demo\n",
        "  Scenario: s\n",
        "    Given a message\n",
        "      ```\n",
        "      hello\n",
        "      ```\n",
    );
    std::fs::write(&path, feature).expect("write feature file");

    let index = index_feature_file(&path).expect("index feature file");
    let step = index.steps.first().expect("expected indexed step");
    let doc = step.docstring.as_ref().expect("doc string present");
    let doc_text = feature
        .get(doc.span.start..doc.span.end)
        .expect("doc span should be valid for source");
    assert!(doc_text.contains("```"));
}
