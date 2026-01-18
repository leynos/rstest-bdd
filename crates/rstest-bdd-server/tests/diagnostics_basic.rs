//! Behavioural tests for on-save diagnostics (unimplemented/unused steps).
//!
//! These tests verify that diagnostics are correctly computed for unimplemented
//! feature steps and unused step definitions. Diagnostics are triggered on
//! file save and published via the LSP protocol.
//!
//! Note: These tests verify the diagnostic computation logic rather than the
//! actual LSP notification publishing, as that requires a full client socket.

mod support;

use rstest::{fixture, rstest};
use rstest_bdd_server::test_support::DiagnosticCheckType;
use support::diagnostics_helpers::basic::{
    assert_feature_has_diagnostic, assert_feature_has_no_diagnostics, assert_rust_has_diagnostic,
    assert_rust_has_no_diagnostics, compute_feature_diagnostics, compute_rust_diagnostics,
};
use support::{ScenarioBuilder, TestScenario};

/// Fixture providing a fresh scenario builder for each test.
#[fixture]
fn scenario_builder() -> ScenarioBuilder {
    ScenarioBuilder::new()
}

#[rstest]
fn feature_with_all_steps_implemented_reports_no_diagnostics(scenario_builder: ScenarioBuilder) {
    let TestScenario { dir, state } = scenario_builder
        .with_feature(
            "test.feature",
            concat!(
                "Feature: test\n",
                "  Scenario: s\n",
                "    Given a step\n",
                "    When something happens\n",
                "    Then result is verified\n",
            ),
        )
        .with_rust_steps(
            "steps.rs",
            concat!(
                "use rstest_bdd_macros::{given, when, then};\n\n",
                "#[given(\"a step\")]\n",
                "fn step() {}\n\n",
                "#[when(\"something happens\")]\n",
                "fn happens() {}\n\n",
                "#[then(\"result is verified\")]\n",
                "fn verified() {}\n",
            ),
        )
        .build();

    assert_feature_has_no_diagnostics(&state, &dir, "test.feature");
}

#[rstest]
fn unimplemented_feature_step_reports_diagnostic(scenario_builder: ScenarioBuilder) {
    let TestScenario { dir, state } = scenario_builder
        .with_feature(
            "test.feature",
            concat!(
                "Feature: test\n",
                "  Scenario: s\n",
                "    Given an unimplemented step\n",
            ),
        )
        .with_rust_steps(
            "steps.rs",
            concat!(
                "use rstest_bdd_macros::given;\n\n",
                "#[given(\"a different step\")]\n",
                "fn diff() {}\n",
            ),
        )
        .build();

    assert_feature_has_diagnostic(&state, &dir, "test.feature", "an unimplemented step");
}

#[rstest]
fn unused_rust_step_reports_diagnostic(scenario_builder: ScenarioBuilder) {
    let TestScenario { dir, state } = scenario_builder
        .with_feature(
            "test.feature",
            concat!("Feature: test\n", "  Scenario: s\n", "    Given a step\n",),
        )
        .with_rust_steps(
            "steps.rs",
            concat!(
                "use rstest_bdd_macros::given;\n\n",
                "#[given(\"a step\")]\n",
                "fn step() {}\n\n",
                "#[given(\"unused step\")]\n",
                "fn unused() {}\n",
            ),
        )
        .build();

    assert_rust_has_diagnostic(&state, &dir, "steps.rs", "unused step");
}

#[rstest]
#[case::rust_all_used(
    "test.feature",
    concat!("Feature: test\n", "  Scenario: s\n", "    Given a step\n",),
    "steps.rs",
    concat!(
        "use rstest_bdd_macros::given;\n\n",
        "#[given(\"a step\")]\n",
        "fn step() {}\n",
    ),
    DiagnosticCheckType::Rust
)]
#[case::parameterized_match(
    "test.feature",
    concat!(
        "Feature: test\n",
        "  Scenario: s\n",
        "    Given I have 42 items\n",
    ),
    "steps.rs",
    concat!(
        "use rstest_bdd_macros::given;\n\n",
        "#[given(\"I have {n:u32} items\")]\n",
        "fn items() {}\n",
    ),
    DiagnosticCheckType::Both
)]
fn no_diagnostics_reported(
    scenario_builder: ScenarioBuilder,
    #[case] feature_filename: &str,
    #[case] feature_content: &str,
    #[case] rust_filename: &str,
    #[case] rust_content: &str,
    #[case] check_type: DiagnosticCheckType,
) {
    let TestScenario { dir, state } = scenario_builder
        .with_feature(feature_filename, feature_content)
        .with_rust_steps(rust_filename, rust_content)
        .build();

    match check_type {
        DiagnosticCheckType::Rust => {
            assert_rust_has_no_diagnostics(&state, &dir, rust_filename);
        }
        DiagnosticCheckType::Feature => {
            assert_feature_has_no_diagnostics(&state, &dir, feature_filename);
        }
        DiagnosticCheckType::Both => {
            assert_feature_has_no_diagnostics(&state, &dir, feature_filename);
            assert_rust_has_no_diagnostics(&state, &dir, rust_filename);
        }
    }
}

#[rstest]
fn keyword_mismatch_produces_diagnostics(scenario_builder: ScenarioBuilder) {
    // Given step should not match When implementation
    let TestScenario { dir, state } = scenario_builder
        .with_feature(
            "test.feature",
            concat!("Feature: test\n", "  Scenario: s\n", "    Given a step\n",),
        )
        .with_rust_steps(
            "steps.rs",
            concat!(
                "use rstest_bdd_macros::when;\n\n",
                "#[when(\"a step\")]\n",
                "fn step() {}\n",
            ),
        )
        .build();

    let feature_diags = compute_feature_diagnostics(&state, &dir, "test.feature");
    let rust_diags = compute_rust_diagnostics(&state, &dir, "steps.rs");

    assert_eq!(
        feature_diags.len(),
        1,
        "Given step should not match When implementation"
    );
    assert_eq!(rust_diags.len(), 1, "When step should be unused");
}

#[rstest]
fn multiple_feature_files_are_checked(scenario_builder: ScenarioBuilder) {
    let TestScenario { dir, state } = scenario_builder
        .with_feature(
            "one.feature",
            concat!("Feature: one\n", "  Scenario: s\n", "    Given step one\n",),
        )
        .with_feature(
            "two.feature",
            concat!("Feature: two\n", "  Scenario: s\n", "    Given step two\n",),
        )
        .with_rust_steps(
            "steps.rs",
            concat!(
                "use rstest_bdd_macros::given;\n\n",
                "#[given(\"step one\")]\n",
                "fn one() {}\n",
            ),
        )
        .build();

    let diags_one = compute_feature_diagnostics(&state, &dir, "one.feature");
    let diags_two = compute_feature_diagnostics(&state, &dir, "two.feature");

    assert!(diags_one.is_empty(), "step one should be implemented");
    assert_eq!(diags_two.len(), 1, "step two should be unimplemented");
}

#[rstest]
fn step_used_in_any_feature_is_not_unused(scenario_builder: ScenarioBuilder) {
    let TestScenario { dir, state } = scenario_builder
        .with_feature(
            "one.feature",
            concat!(
                "Feature: one\n",
                "  Scenario: s\n",
                "    Given a shared step\n",
            ),
        )
        .with_feature(
            "two.feature",
            concat!(
                "Feature: two\n",
                "  Scenario: s\n",
                "    Given another step\n",
            ),
        )
        .with_rust_steps(
            "steps.rs",
            concat!(
                "use rstest_bdd_macros::given;\n\n",
                "#[given(\"a shared step\")]\n",
                "fn shared() {}\n",
            ),
        )
        .build();

    let rust_diags = compute_rust_diagnostics(&state, &dir, "steps.rs");

    assert!(
        rust_diags.is_empty(),
        "step used in at least one feature should not be unused"
    );
}
