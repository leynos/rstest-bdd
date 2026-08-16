//! Diagnostics tests for scenario outline column validation.
//!
//! This module verifies that `super::scenario_outline` reports missing and
//! surplus Examples-table columns for scenario-outline placeholders. It is
//! part of the parent diagnostics test module and uses its shared
//! scenario-building infrastructure.

use super::*;

/// What a scenario-outline column case expects to see reported.
struct OutlineExpectation<'a> {
    /// Number of diagnostics expected for the staged feature.
    count: usize,
    /// Diagnostic code expected on the single diagnostic, when one is expected.
    code: Option<&'a str>,
    /// Fragment the diagnostic message must contain, when a code is expected.
    message_fragment: Option<&'a str>,
    /// Diagnostics that must each appear, irrespective of their order.
    diagnostics: &'a [ExpectedDiagnostic<'a>],
}

/// A diagnostic code and message fragment expected from a column case.
struct ExpectedDiagnostic<'a> {
    /// Diagnostic code that identifies the reported problem.
    code: &'a str,
    /// Fragment that identifies the affected column in the diagnostic message.
    message_fragment: &'a str,
}

/// Helper to compute scenario outline column diagnostics.
fn compute_scenario_outline_diagnostics_for_path(
    state: &ServerState,
    feature_path: &Path,
) -> std::io::Result<Vec<Diagnostic>> {
    let feature_index = state.feature_index(feature_path).ok_or_else(|| {
        std::io::Error::other(format!(
            "feature index missing for {}",
            feature_path.display()
        ))
    })?;
    Ok(scenario_outline::compute_scenario_outline_column_diagnostics(feature_index))
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
    OutlineExpectation {
        count: 1,
        code: Some(CODE_EXAMPLE_COLUMN_MISSING),
        message_fragment: Some("type"),
        diagnostics: &[],
    },
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
    OutlineExpectation {
        count: 1,
        code: Some(CODE_EXAMPLE_COLUMN_SURPLUS),
        message_fragment: Some("unused"),
        diagnostics: &[],
    },
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
    OutlineExpectation {
        count: 0,
        code: None,
        message_fragment: None,
        diagnostics: &[],
    },
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
    OutlineExpectation {
        count: 0,
        code: None,
        message_fragment: None,
        diagnostics: &[],
    },
)]
#[case::missing_and_surplus(
    // Step uses <type>, Examples has an extra | unused | column.
    concat!(
        "Feature: test\n",
        "  Scenario Outline: outline\n",
        "    Given I have <count> <type> items\n",
        "    Examples:\n",
        "      | count | unused |\n",
        "      | 5     | value  |\n",
    ),
    // Both missing (type) and surplus (unused).
    OutlineExpectation {
        count: 2,
        code: None,
        message_fragment: None,
        diagnostics: &[
            ExpectedDiagnostic {
                code: CODE_EXAMPLE_COLUMN_MISSING,
                message_fragment: "type",
            },
            ExpectedDiagnostic {
                code: CODE_EXAMPLE_COLUMN_SURPLUS,
                message_fragment: "unused",
            },
        ],
    },
)]
fn scenario_outline_column_validation(
    scenario_builder: ScenarioBuilder,
    #[case] feature_content: &str,
    #[case] expected: OutlineExpectation<'_>,
) {
    let OutlineExpectation {
        count: expected_count,
        code: expected_code,
        message_fragment: expected_message_fragment,
        diagnostics: expected_diagnostics,
    } = expected;
    // Use just the feature file - no Rust code needed for column validation
    let scenario = scenario_builder.with_single_file_pair(
        feature_content,
        // Minimal Rust content to satisfy the builder
        "// no step definitions needed\n",
    );
    let diagnostics = match compute_scenario_outline_diagnostics_for_path(
        &scenario.state,
        &scenario.feature_path,
    ) {
        Ok(found) => found,
        Err(error) => panic!("failed to compute scenario outline diagnostics: {error}"),
    };

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

    for expected_diagnostic in expected_diagnostics {
        let expected_code = lsp_types::NumberOrString::String(expected_diagnostic.code.to_owned());
        let Some(diagnostic) = diagnostics
            .iter()
            .find(|diagnostic| diagnostic.code.as_ref() == Some(&expected_code))
        else {
            panic!(
                "expected a diagnostic with code '{}' in {diagnostics:#?}",
                expected_diagnostic.code
            );
        };
        assert_diagnostic_message_contains(diagnostic, &[expected_diagnostic.message_fragment]);
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
        compute_scenario_outline_diagnostics_for_path(&scenario.state, &scenario.feature_path)
            .expect("test setup should succeed");
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
    let diagnostics = match compute_scenario_outline_diagnostics_for_path(
        &scenario.state,
        &scenario.feature_path,
    ) {
        Ok(found) => found,
        Err(error) => panic!("failed to compute scenario outline diagnostics: {error}"),
    };
    assert!(diagnostics.is_empty(), "{assertion_message}");
}
