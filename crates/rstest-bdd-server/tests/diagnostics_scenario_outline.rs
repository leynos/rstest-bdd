//! Behavioural tests for scenario outline column validation diagnostics.
//!
//! These tests verify that diagnostics are correctly emitted when scenario
//! outline placeholders (`<column>`) don't match Examples table column headers.

mod support;

use rstest::{fixture, rstest};
use rstest_bdd_server::handlers::compute_scenario_outline_column_diagnostics;
use rstest_bdd_server::server::ServerState;
use support::{ScenarioBuilder, TestScenario};
use tempfile::TempDir;

/// Fixture providing a fresh scenario builder for each test.
#[fixture]
fn scenario_builder() -> ScenarioBuilder {
    ScenarioBuilder::new()
}

// ─────────────────────────────────────────────────────────────────────────────
// Test-local helpers
// ─────────────────────────────────────────────────────────────────────────────

/// Helper to compute scenario outline column diagnostics for a feature file.
#[expect(clippy::expect_used, reason = "test helper uses expect for clarity")]
fn compute_outline_column_diagnostics(
    state: &ServerState,
    dir: &TempDir,
    filename: impl AsRef<str>,
) -> Vec<lsp_types::Diagnostic> {
    let path = dir.path().join(filename.as_ref());
    let feature_index = state.feature_index(&path).expect("feature index");
    compute_scenario_outline_column_diagnostics(feature_index)
}

/// Helper to build a test scenario and compute diagnostics.
fn setup_and_compute_diagnostics(
    scenario_builder: ScenarioBuilder,
    feature_content: &str,
) -> Vec<lsp_types::Diagnostic> {
    let TestScenario { dir, state } = scenario_builder
        .with_feature("test.feature", feature_content)
        .with_rust_steps("steps.rs", "// no step definitions\n")
        .build();
    compute_outline_column_diagnostics(&state, &dir, "test.feature")
}

// ─────────────────────────────────────────────────────────────────────────────
// Tests
// ─────────────────────────────────────────────────────────────────────────────

#[rstest]
#[case::missing_column(
    // Step uses <count> but Examples only has | name |
    concat!(
        "Feature: test\n",
        "  Scenario Outline: outline\n",
        "    Given I have <count> <name> items\n",
        "    Examples:\n",
        "      | name |\n",
        "      | foo  |\n",
    ),
    1,
    Some("count"),
    Some("no matching column"),
)]
#[case::surplus_column(
    // Examples has | unused | but steps don't reference it
    concat!(
        "Feature: test\n",
        "  Scenario Outline: outline\n",
        "    Given I have <count> items\n",
        "    Examples:\n",
        "      | count | unused |\n",
        "      | 5     | x      |\n",
    ),
    1,
    Some("unused"),
    Some("not referenced"),
)]
#[case::matched_columns(
    // All placeholders match Examples columns - no diagnostic
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
#[case::multiple_matched_columns(
    // Multiple placeholders all match - no diagnostic
    concat!(
        "Feature: test\n",
        "  Scenario Outline: outline\n",
        "    Given I have <count> <type> items\n",
        "    When I add <more> items\n",
        "    Examples:\n",
        "      | count | type | more |\n",
        "      | 5     | red  | 3    |\n",
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
    #[case] expected_message_fragment: Option<&str>,
    #[case] secondary_fragment: Option<&str>,
) {
    let diagnostics = setup_and_compute_diagnostics(scenario_builder, feature_content);

    assert_eq!(
        diagnostics.len(),
        expected_count,
        "expected {expected_count} diagnostic(s), got {}: {:?}",
        diagnostics.len(),
        diagnostics.iter().map(|d| &d.message).collect::<Vec<_>>()
    );

    if let Some(fragment) = expected_message_fragment {
        let has_match = diagnostics.iter().any(|d| d.message.contains(fragment));
        assert!(
            has_match,
            "expected a diagnostic message containing '{fragment}'"
        );
    }

    if let Some(secondary) = secondary_fragment {
        let has_match = diagnostics.iter().any(|d| d.message.contains(secondary));
        assert!(
            has_match,
            "expected a diagnostic message containing '{secondary}'"
        );
    }
}

/// Tests for scenarios that should produce no column diagnostics.
#[rstest]
#[case::regular_scenario(
    // Regular scenarios (not outlines) should produce no column diagnostics
    concat!(
        "Feature: test\n",
        "  Scenario: regular\n",
        "    Given a step\n",
    ),
)]
#[case::placeholder_in_docstring(
    // Placeholders in docstrings should also be matched against columns
    concat!(
        "Feature: test\n",
        "  Scenario Outline: outline\n",
        "    Given a message\n",
        "      \"\"\"\n",
        "      Hello <name>\n",
        "      \"\"\"\n",
        "    Examples:\n",
        "      | name  |\n",
        "      | World |\n",
    ),
)]
#[case::placeholder_in_table_cell(
    // Placeholders in data table cells should also be matched against columns
    concat!(
        "Feature: test\n",
        "  Scenario Outline: outline\n",
        "    Given a table\n",
        "      | key  | value   |\n",
        "      | item | <value> |\n",
        "    Examples:\n",
        "      | value |\n",
        "      | 42    |\n",
    ),
)]
#[case::empty_examples(
    // An Examples block without a table should not cause a crash
    concat!(
        "Feature: test\n",
        "  Scenario Outline: outline\n",
        "    Given the system has <value> items\n",
        "    Examples:\n",
    ),
)]
#[case::header_only_examples(
    // An Examples table with only a header row (no body rows) should not crash
    concat!(
        "Feature: test\n",
        "  Scenario Outline: outline\n",
        "    Given the system has <value> items\n",
        "    Examples:\n",
        "      | value |\n",
    ),
)]
#[case::background_placeholder_with_matching_column(
    // Background placeholder with matching column should not produce diagnostic
    concat!(
        "Feature: test\n",
        "  Background:\n",
        "    Given the environment is <env>\n",
        "  Scenario Outline: outline\n",
        "    Given the system has <count> items\n",
        "    Examples:\n",
        "      | env  | count |\n",
        "      | prod | 5     |\n",
    ),
)]
fn no_diagnostics_scenarios(scenario_builder: ScenarioBuilder, #[case] feature_content: &str) {
    let diagnostics = setup_and_compute_diagnostics(scenario_builder, feature_content);
    assert!(
        diagnostics.is_empty(),
        "expected no diagnostics, got: {diagnostics:?}"
    );
}

#[rstest]
fn multiple_examples_tables_validated_independently(scenario_builder: ScenarioBuilder) {
    // Each Examples table should be validated independently
    let diagnostics = setup_and_compute_diagnostics(
        scenario_builder,
        concat!(
            "Feature: test\n",
            "  Scenario Outline: outline\n",
            "    Given I have <count> items\n",
            "    Examples: complete\n",
            "      | count |\n",
            "      | 5     |\n",
            "    Examples: missing column\n",
            "      | other |\n",
            "      | x     |\n",
        ),
    );
    // Second table has both: missing 'count' and surplus 'other'
    assert_eq!(
        diagnostics.len(),
        2,
        "second Examples table should produce diagnostics"
    );
}

#[rstest]
fn outline_inside_rule_validates_columns(scenario_builder: ScenarioBuilder) {
    // Scenario Outlines inside Rules should be validated
    let diagnostics = setup_and_compute_diagnostics(
        scenario_builder,
        concat!(
            "Feature: test\n",
            "  Rule: business rule\n",
            "    Scenario Outline: outline in rule\n",
            "      Given the system has <count> items\n",
            "      Examples:\n",
            "        | other |\n",
            "        | 5     |\n",
        ),
    );
    // Missing 'count' and surplus 'other'
    assert_eq!(
        diagnostics.len(),
        2,
        "outline inside Rule should produce diagnostics"
    );
    assert!(
        diagnostics.iter().any(|d| d.message.contains("count")),
        "should report missing 'count' column"
    );
    assert!(
        diagnostics.iter().any(|d| d.message.contains("other")),
        "should report surplus 'other' column"
    );
}

#[rstest]
fn multiple_outlines_diagnostics_scoped_correctly(scenario_builder: ScenarioBuilder) {
    // Multiple outlines - diagnostics should not leak between them
    let diagnostics = setup_and_compute_diagnostics(
        scenario_builder,
        concat!(
            "Feature: test\n",
            "  Scenario Outline: first outline\n",
            "    Given the system has <alpha> items\n",
            "    Examples:\n",
            "      | alpha |\n",
            "      | 1     |\n",
            "  Scenario Outline: second outline\n",
            "    Given the system has <beta> items\n",
            "    Examples:\n",
            "      | gamma |\n",
            "      | 2     |\n",
        ),
    );
    // First outline is correct, second has missing 'beta' and surplus 'gamma'
    assert_eq!(
        diagnostics.len(),
        2,
        "only second outline should produce diagnostics"
    );
    // Ensure diagnostics relate to second outline, not first
    assert!(
        diagnostics.iter().any(|d| d.message.contains("beta")),
        "should report missing 'beta' from second outline"
    );
    assert!(
        diagnostics.iter().any(|d| d.message.contains("gamma")),
        "should report surplus 'gamma' from second outline"
    );
    assert!(
        !diagnostics.iter().any(|d| d.message.contains("alpha")),
        "first outline's column 'alpha' should not appear in diagnostics"
    );
}

#[rstest]
#[case::feature_background(
    // Placeholders in feature Background should be validated against Examples
    concat!(
        "Feature: test\n",
        "  Background:\n",
        "    Given the environment is <env>\n",
        "  Scenario Outline: outline\n",
        "    Given the system has <count> items\n",
        "    Examples:\n",
        "      | count |\n",
        "      | 5     |\n",
    ),
    "env",
    "feature background placeholder without matching column should produce diagnostic",
)]
#[case::rule_background(
    // Placeholders in Rule Background should be validated against Examples
    concat!(
        "Feature: test\n",
        "  Rule: business rule\n",
        "    Background:\n",
        "      Given the context is <context>\n",
        "    Scenario Outline: outline in rule\n",
        "      Given the system has <count> items\n",
        "      Examples:\n",
        "        | count |\n",
        "        | 5     |\n",
    ),
    "context",
    "rule background placeholder without matching column should produce diagnostic",
)]
fn background_placeholder_requires_column(
    scenario_builder: ScenarioBuilder,
    #[case] feature_content: &str,
    #[case] missing_placeholder: &str,
    #[case] assertion_message: &str,
) {
    let diagnostics = setup_and_compute_diagnostics(scenario_builder, feature_content);
    assert_eq!(diagnostics.len(), 1, "{assertion_message}");
    assert!(
        diagnostics
            .iter()
            .any(|d| d.message.contains(missing_placeholder)),
        "diagnostic should mention missing '{missing_placeholder}' column"
    );
}
