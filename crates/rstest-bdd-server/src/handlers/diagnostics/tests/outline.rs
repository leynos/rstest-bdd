//! Diagnostics tests for scenario outline column validation.

use super::*;

/// Helper to compute scenario outline column diagnostics.
#[expect(
    clippy::expect_used,
    reason = "test helper requires explicit panic for debugging failures"
)]
fn compute_scenario_outline_diagnostics_for_path(
    state: &ServerState,
    feature_path: &Path,
) -> Vec<Diagnostic> {
    let feature_index = state.feature_index(feature_path).expect("feature index");
    scenario_outline::compute_scenario_outline_column_diagnostics(feature_index)
}

#[rstest]
#[case::missing_column_only(
    // Step uses <count> but Examples has | count | plus missing <type>
    concat!(
        "Feature: test\n",
        "  Scenario Outline: outline\n",
        "    Given I have <count> <type> items\n",
        "    Examples:\n",
        "      | count |\n",
        "      | 5     |\n",
    ),
    1,
    Some(CODE_EXAMPLE_COLUMN_MISSING),
    Some("type"),
)]
#[case::surplus_column_only(
    // Examples has extra | unused | column not referenced by steps
    concat!(
        "Feature: test\n",
        "  Scenario Outline: outline\n",
        "    Given I have <count> items\n",
        "    Examples:\n",
        "      | count | unused |\n",
        "      | 5     | value  |\n",
    ),
    1,
    Some(CODE_EXAMPLE_COLUMN_SURPLUS),
    Some("unused"),
)]
#[case::matched_columns(
    // <count> matches | count |
    concat!(
        "Feature: test\n",
        "  Scenario Outline: outline\n",
        "    Given I have <count> items\n",
        "    Examples:\n",
        "      | count |\n",
        "      | 5     |\n",
    ),
    0,
    None,
    None,
)]
#[case::multiple_placeholders_matched(
    // <count> and <type> both match columns
    concat!(
        "Feature: test\n",
        "  Scenario Outline: outline\n",
        "    Given I have <count> <type> items\n",
        "    Examples:\n",
        "      | count | type  |\n",
        "      | 5     | red   |\n",
    ),
    0,
    None,
    None,
)]
#[case::missing_and_surplus(
    // Step uses <count>, Examples has | other | - both issues
    concat!(
        "Feature: test\n",
        "  Scenario Outline: outline\n",
        "    Given I have <count> items\n",
        "    Examples:\n",
        "      | other |\n",
        "      | value |\n",
    ),
    // Both missing (count) and surplus (other)
    2,
    None,
    None,
)]
fn scenario_outline_column_validation(
    scenario_builder: ScenarioBuilder,
    #[case] feature_content: &str,
    #[case] expected_count: usize,
    #[case] expected_code: Option<&str>,
    #[case] expected_message_fragment: Option<&str>,
) {
    // Use just the feature file - no Rust code needed for column validation
    let scenario = scenario_builder.with_single_file_pair(
        feature_content,
        // Minimal Rust content to satisfy the builder
        "// no step definitions needed\n",
    );
    let diagnostics =
        compute_scenario_outline_diagnostics_for_path(&scenario.state, &scenario.feature_path);

    assert_eq!(
        diagnostics.len(),
        expected_count,
        "expected {expected_count} diagnostic(s), found {}",
        diagnostics.len()
    );

    if let Some(code) = expected_code {
        let diag = assert_single_diagnostic_with_code(&diagnostics, code);
        if let Some(fragment) = expected_message_fragment {
            assert_diagnostic_message_contains(diag, &[fragment]);
        }
    }
}

#[rstest]
fn regular_scenario_no_column_diagnostics(scenario_builder: ScenarioBuilder) {
    // Regular scenarios (not outlines) should not produce column diagnostics
    let scenario = scenario_builder.with_single_file_pair(
        concat!(
            "Feature: test\n",
            "  Scenario: regular\n",
            "    Given a step\n",
        ),
        "// no step definitions\n",
    );
    let diagnostics =
        compute_scenario_outline_diagnostics_for_path(&scenario.state, &scenario.feature_path);
    assert!(
        diagnostics.is_empty(),
        "regular scenarios should produce no column diagnostics"
    );
}

#[rstest]
#[case::docstring(
    concat!(
        "Feature: test\n",
        "  Scenario Outline: outline\n",
        "    Given a message\n",
        "      \"\"\"\n",
        "      Hello <name>\n",
        "      \"\"\"\n",
        "    Examples:\n",
        "      | name |\n",
        "      | World |\n",
    ),
    "placeholder in docstring should match column"
)]
#[case::table_cell(
    concat!(
        "Feature: test\n",
        "  Scenario Outline: outline\n",
        "    Given a table\n",
        "      | key   | value   |\n",
        "      | item  | <value> |\n",
        "    Examples:\n",
        "      | value |\n",
        "      | 42    |\n",
    ),
    "placeholder in table cell should match column"
)]
fn placeholder_detected_in_various_contexts(
    scenario_builder: ScenarioBuilder,
    #[case] feature_content: &str,
    #[case] assertion_message: &str,
) {
    let scenario =
        scenario_builder.with_single_file_pair(feature_content, "// no step definitions\n");
    let diagnostics =
        compute_scenario_outline_diagnostics_for_path(&scenario.state, &scenario.feature_path);
    assert!(diagnostics.is_empty(), "{assertion_message}");
}
