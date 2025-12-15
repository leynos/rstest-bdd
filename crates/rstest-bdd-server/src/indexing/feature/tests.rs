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
