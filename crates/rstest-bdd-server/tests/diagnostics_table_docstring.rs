//! Behavioural tests for table/docstring expectation mismatch diagnostics.
//!
//! These tests verify that diagnostics are correctly emitted when a step
//! definition's data table or docstring expectations don't match what's
//! provided in the feature file.

mod support;

use rstest::{fixture, rstest};
use support::diagnostics_helpers::compute_table_docstring_diagnostics;
use support::{ScenarioBuilder, TestScenario};

/// Fixture providing a fresh scenario builder for each test.
#[fixture]
fn scenario_builder() -> ScenarioBuilder {
    ScenarioBuilder::new()
}

#[rstest]
#[case::table_not_expected(
    // Feature has table, Rust doesn't expect it
    concat!(
        "Feature: test\n",
        "  Scenario: s\n",
        "    Given a step\n",
        "      | col |\n",
        "      | val |\n",
    ),
    concat!(
        "use rstest_bdd_macros::given;\n\n",
        "#[given(\"a step\")]\n",
        "fn a_step() {}\n",
    ),
    1,
    Some("does not expect"),
)]
#[case::table_expected(
    // Rust expects table, feature doesn't have one
    concat!("Feature: test\n", "  Scenario: s\n", "    Given a step\n"),
    concat!(
        "use rstest_bdd_macros::given;\n",
        "use rstest_bdd::DataTable;\n\n",
        "#[given(\"a step\")]\n",
        "fn a_step(datatable: DataTable) {}\n",
    ),
    1,
    Some("expects a data table"),
)]
#[case::docstring_not_expected(
    // Feature has docstring, Rust doesn't expect it
    concat!(
        "Feature: test\n",
        "  Scenario: s\n",
        "    Given a step\n",
        "      \"\"\"\n",
        "      content\n",
        "      \"\"\"\n",
    ),
    concat!(
        "use rstest_bdd_macros::given;\n\n",
        "#[given(\"a step\")]\n",
        "fn a_step() {}\n",
    ),
    1,
    Some("does not expect"),
)]
#[case::docstring_expected(
    // Rust expects docstring, feature doesn't have one
    concat!("Feature: test\n", "  Scenario: s\n", "    Given a step\n"),
    concat!(
        "use rstest_bdd_macros::given;\n\n",
        "#[given(\"a step\")]\n",
        "fn a_step(docstring: String) {}\n",
    ),
    1,
    Some("expects a doc string"),
)]
#[case::matched_table(
    // Both feature and Rust have table - no diagnostic
    concat!(
        "Feature: test\n",
        "  Scenario: s\n",
        "    Given a step\n",
        "      | col |\n",
        "      | val |\n",
    ),
    concat!(
        "use rstest_bdd_macros::given;\n",
        "use rstest_bdd::DataTable;\n\n",
        "#[given(\"a step\")]\n",
        "fn a_step(datatable: DataTable) {}\n",
    ),
    0,
    None,
)]
#[case::matched_docstring(
    // Both feature and Rust have docstring - no diagnostic
    concat!(
        "Feature: test\n",
        "  Scenario: s\n",
        "    Given a step\n",
        "      \"\"\"\n",
        "      some content\n",
        "      \"\"\"\n",
    ),
    concat!(
        "use rstest_bdd_macros::given;\n\n",
        "#[given(\"a step\")]\n",
        "fn a_step(docstring: String) {}\n",
    ),
    0,
    None,
)]
fn table_docstring_validation(
    scenario_builder: ScenarioBuilder,
    #[case] feature_content: &str,
    #[case] rust_content: &str,
    #[case] expected_count: usize,
    #[case] message_substring: Option<&str>,
) {
    let TestScenario { dir, state } = scenario_builder
        .with_feature("test.feature", feature_content)
        .with_rust_steps("steps.rs", rust_content)
        .build();

    let diagnostics = compute_table_docstring_diagnostics(&state, &dir, "test.feature");

    assert_eq!(
        diagnostics.len(),
        expected_count,
        "expected {expected_count} diagnostic(s)"
    );
    if let Some(substring) = message_substring {
        #[expect(clippy::expect_used, reason = "checked count > 0 in conditional")]
        let diag = diagnostics.first().expect("checked count > 0");
        assert!(
            diag.message.contains(substring),
            "diagnostic message should contain '{substring}'"
        );
    }
}
