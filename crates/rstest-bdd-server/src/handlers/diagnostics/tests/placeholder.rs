//! Placeholder count mismatch diagnostic tests.

use super::*;

/// Helper to compute signature mismatch diagnostics.
fn compute_signature_diagnostics_for_path(
    state: &ServerState,
    rust_path: &Path,
) -> Vec<Diagnostic> {
    placeholder::compute_signature_mismatch_diagnostics(state, rust_path)
}

#[rstest]
#[case::missing_param(
    // Pattern has 1 placeholder, function has 0 step arguments
    "Feature: test\n  Scenario: s\n    Given I have 5 apples\n",
    concat!(
        "use rstest_bdd_macros::given;\n\n",
        "#[given(\"I have {count} apples\")]\n",
        "fn have_apples() {}\n",
    ),
    1,
    Some(("1 placeholder", "0 step argument")),
)]
#[case::extra_placeholder(
    // Pattern has 2 placeholders, function has 1 step argument
    "Feature: test\n  Scenario: s\n    Given I have 5 red apples\n",
    concat!(
        "use rstest_bdd_macros::given;\n\n",
        "#[given(\"I have {count} {color} apples\")]\n",
        "fn have_apples(count: u32) {}\n",
    ),
    1,
    Some(("2 placeholder", "1 step argument")),
)]
#[case::counts_match(
    // Pattern has 1 placeholder, function has 1 step argument - no diagnostic
    "Feature: test\n  Scenario: s\n    Given I have 5 apples\n",
    concat!(
        "use rstest_bdd_macros::given;\n\n",
        "#[given(\"I have {count} apples\")]\n",
        "fn have_apples(count: u32) {}\n",
    ),
    0,
    None,
)]
#[case::fixture_excluded(
    // Pattern has 1 placeholder, function has 1 step arg + 1 fixture - no diagnostic
    "Feature: test\n  Scenario: s\n    Given I have 5 apples\n",
    concat!(
        "use rstest_bdd_macros::given;\n\n",
        "#[given(\"I have {count} apples\")]\n",
        "fn have_apples(count: u32, context: &mut TestContext) {}\n",
    ),
    0,
    None,
)]
#[case::datatable_docstring_excluded(
    // Pattern has 1 placeholder, function has count + datatable + docstring - no diagnostic
    "Feature: test\n  Scenario: s\n    Given I have 5 apples\n      | col |\n      | val |\n",
    concat!(
        "use rstest_bdd_macros::given;\n",
        "use rstest_bdd::DataTable;\n\n",
        "#[given(\"I have {count} apples\")]\n",
        "fn have_apples(count: u32, datatable: DataTable, docstring: String) {}\n",
    ),
    0,
    None,
)]
fn placeholder_count_validation(
    scenario_builder: ScenarioBuilder,
    #[case] feature_content: &str,
    #[case] rust_content: &str,
    #[case] expected_count: usize,
    #[case] message_fragments: Option<(&str, &str)>,
) {
    let scenario = scenario_builder.with_single_file_pair(feature_content, rust_content);
    let diagnostics = compute_signature_diagnostics_for_path(&scenario.state, &scenario.rust_path);

    assert_eq!(
        diagnostics.len(),
        expected_count,
        "expected {expected_count} diagnostic(s)"
    );
    if let Some((frag1, frag2)) = message_fragments {
        let diag =
            assert_single_diagnostic_with_code(&diagnostics, CODE_PLACEHOLDER_COUNT_MISMATCH);
        assert_diagnostic_message_contains(diag, &[frag1, frag2]);
    }
}
