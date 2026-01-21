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
    let TestScenario { dir, state } = scenario_builder
        .with_feature("test.feature", feature_content)
        // No Rust steps needed for column validation
        .with_rust_steps("steps.rs", "// no step definitions\n")
        .build();

    let diagnostics = compute_outline_column_diagnostics(&state, &dir, "test.feature");

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

#[rstest]
fn regular_scenario_no_column_diagnostics(scenario_builder: ScenarioBuilder) {
    // Regular scenarios (not outlines) should produce no column diagnostics
    let TestScenario { dir, state } = scenario_builder
        .with_feature(
            "test.feature",
            concat!(
                "Feature: test\n",
                "  Scenario: regular\n",
                "    Given a step\n",
            ),
        )
        .with_rust_steps("steps.rs", "// no step definitions\n")
        .build();

    let diagnostics = compute_outline_column_diagnostics(&state, &dir, "test.feature");
    assert!(
        diagnostics.is_empty(),
        "regular scenarios should produce no column diagnostics"
    );
}

#[rstest]
fn placeholder_in_docstring_matches_column(scenario_builder: ScenarioBuilder) {
    // Placeholders in docstrings should also be matched against columns
    let TestScenario { dir, state } = scenario_builder
        .with_feature(
            "test.feature",
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
        )
        .with_rust_steps("steps.rs", "// no step definitions\n")
        .build();

    let diagnostics = compute_outline_column_diagnostics(&state, &dir, "test.feature");
    assert!(
        diagnostics.is_empty(),
        "placeholder in docstring should match column"
    );
}

#[rstest]
fn placeholder_in_table_cell_matches_column(scenario_builder: ScenarioBuilder) {
    // Placeholders in data table cells should also be matched against columns
    let TestScenario { dir, state } = scenario_builder
        .with_feature(
            "test.feature",
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
        )
        .with_rust_steps("steps.rs", "// no step definitions\n")
        .build();

    let diagnostics = compute_outline_column_diagnostics(&state, &dir, "test.feature");
    assert!(
        diagnostics.is_empty(),
        "placeholder in table cell should match column"
    );
}

#[rstest]
fn multiple_examples_tables_validated_independently(scenario_builder: ScenarioBuilder) {
    // Each Examples table should be validated independently
    let TestScenario { dir, state } = scenario_builder
        .with_feature(
            "test.feature",
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
        )
        .with_rust_steps("steps.rs", "// no step definitions\n")
        .build();

    let diagnostics = compute_outline_column_diagnostics(&state, &dir, "test.feature");
    // Second table has both: missing 'count' and surplus 'other'
    assert_eq!(
        diagnostics.len(),
        2,
        "second Examples table should produce diagnostics"
    );
}
