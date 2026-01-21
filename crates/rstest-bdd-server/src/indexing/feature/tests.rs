//! Tests for feature file indexing.

use super::*;
use tempfile::TempDir;

#[expect(
    clippy::expect_used,
    reason = "tests use explicit failures for clarity"
)]
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
    assert_eq!(index.steps.len(), 3);
    assert_eq!(index.example_columns.len(), 2);
    let first_column = index
        .example_columns
        .first()
        .expect("expected example columns");
    assert_eq!(first_column.name, "Result");
    let second_column = index
        .example_columns
        .get(1)
        .expect("expected second example column");
    assert_eq!(second_column.name, "Extra");

    let given = index.steps.first().expect("expected indexed steps");
    assert_eq!(given.keyword.trim(), "Given");
    assert!(given.docstring.is_some());
    let doc = given.docstring.as_ref().expect("doc string present");
    assert!(doc.span.start < doc.span.end);

    let when = index.steps.get(1).expect("expected second step");
    assert_eq!(when.keyword.trim(), "When");
    assert!(when.table.is_some());
    let table = when.table.as_ref().expect("table present");
    let first_row = table.rows.first().expect("table should have rows");
    assert_eq!(first_row, &vec!["a".to_string(), "b".to_string()]);
    assert!(table.span.start < table.span.end);

    // Verify scenario outline indexing
    assert_eq!(index.scenario_outlines.len(), 1);
    let outline = index
        .scenario_outlines
        .first()
        .expect("expected scenario outline");
    assert_eq!(outline.name, "outline");
    assert_eq!(outline.step_indices, vec![0, 1, 2]);
    assert_eq!(outline.examples.len(), 1);

    let examples_table = outline.examples.first().expect("expected examples table");
    assert_eq!(examples_table.columns.len(), 2);
    let first_col = examples_table.columns.first().expect("first column");
    let second_col = examples_table.columns.get(1).expect("second column");
    assert_eq!(first_col.name, "Result");
    assert_eq!(second_col.name, "Extra");
}

#[expect(
    clippy::expect_used,
    reason = "tests use explicit failures for clarity"
)]
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

#[expect(
    clippy::expect_used,
    reason = "tests use explicit failures for clarity"
)]
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

#[expect(
    clippy::expect_used,
    reason = "tests use explicit failures for clarity"
)]
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
