//! Diagnostics tests for table and docstring expectation mismatches.

use super::*;

/// Helper to compute table/docstring mismatch diagnostics.
#[expect(
    clippy::expect_used,
    reason = "test helper requires explicit panic for debugging failures"
)]
fn compute_table_docstring_diagnostics_for_path(
    state: &ServerState,
    feature_path: &Path,
) -> Vec<Diagnostic> {
    let feature_index = state.feature_index(feature_path).expect("feature index");
    table_docstring::compute_table_docstring_mismatch_diagnostics(state, feature_index)
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
    Some(CODE_TABLE_NOT_EXPECTED),
)]
#[case::table_expected(
    // Rust expects table, feature doesn't have one
    "Feature: test\n  Scenario: s\n    Given a step\n",
    concat!(
        "use rstest_bdd_macros::given;\n",
        "use rstest_bdd::DataTable;\n\n",
        "#[given(\"a step\")]\n",
        "fn a_step(datatable: DataTable) {}\n",
    ),
    Some(CODE_TABLE_EXPECTED),
)]
#[case::table_matched(
    // Both have table - no diagnostic
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
    None,
)]
#[case::docstring_not_expected(
    // Feature has docstring, Rust doesn't expect it
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
        "fn a_step() {}\n",
    ),
    Some(CODE_DOCSTRING_NOT_EXPECTED),
)]
#[case::docstring_expected(
    // Rust expects docstring, feature doesn't have one
    "Feature: test\n  Scenario: s\n    Given a step\n",
    concat!(
        "use rstest_bdd_macros::given;\n\n",
        "#[given(\"a step\")]\n",
        "fn a_step(docstring: String) {}\n",
    ),
    Some(CODE_DOCSTRING_EXPECTED),
)]
fn table_docstring_validation(
    scenario_builder: ScenarioBuilder,
    #[case] feature_content: &str,
    #[case] rust_content: &str,
    #[case] expected_code: Option<&str>,
) {
    let scenario = scenario_builder.with_single_file_pair(feature_content, rust_content);
    let diagnostics =
        compute_table_docstring_diagnostics_for_path(&scenario.state, &scenario.feature_path);

    match expected_code {
        Some(code) => {
            assert_single_diagnostic_with_code(&diagnostics, code);
        }
        None => {
            assert!(
                diagnostics.is_empty(),
                "expected no diagnostics, found {}",
                diagnostics.len()
            );
        }
    }
}

// ========================================================================
// Scenario outline column validation tests
// ========================================================================
