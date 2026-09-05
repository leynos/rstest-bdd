//! Diagnostics tests for unimplemented and unused step detection.

use super::*;

#[rstest]
fn unimplemented_step_produces_diagnostic(scenario_builder: ScenarioBuilder) {
    let scenario = scenario_builder.with_single_file_pair(
        "Feature: test\n  Scenario: s\n    Given an unimplemented step\n",
        concat!(
            "use rstest_bdd_macros::given;\n\n",
            "#[given(\"a different step\")]\n",
            "fn diff() {}\n",
        ),
    );

    let feature_index = scenario
        .state
        .feature_index(&scenario.feature_path)
        .ok_or_else(|| std::io::Error::other("feature index missing"))
        .expect("test setup should succeed");
    let diagnostics = compute_unimplemented_step_diagnostics(&scenario.state, feature_index);

    let [diag] = diagnostics.as_slice() else {
        panic!(
            "expected exactly one diagnostic, found {}",
            diagnostics.len()
        );
    };
    assert_eq!(diag.severity, Some(DiagnosticSeverity::WARNING));
    assert!(diag.message.contains("an unimplemented step"));
    assert_eq!(
        diag.code,
        Some(lsp_types::NumberOrString::String(
            CODE_UNIMPLEMENTED_STEP.to_owned()
        ))
    );
}

#[rstest]
fn unused_step_definition_produces_diagnostic(scenario_builder: ScenarioBuilder) {
    let scenario = scenario_builder.with_single_file_pair(
        "Feature: test\n  Scenario: s\n    Given a step\n",
        concat!(
            "use rstest_bdd_macros::given;\n\n",
            "#[given(\"a step\")]\n",
            "fn step() {}\n\n",
            "#[given(\"unused step\")]\n",
            "fn unused() {}\n",
        ),
    );

    let diagnostics = compute_unused_step_diagnostics(&scenario.state, &scenario.rust_path);

    let [diag] = diagnostics.as_slice() else {
        panic!(
            "expected exactly one diagnostic, found {}",
            diagnostics.len()
        );
    };
    assert!(diag.message.contains("unused step"));
    assert_eq!(
        diag.code,
        Some(lsp_types::NumberOrString::String(
            CODE_UNUSED_STEP_DEFINITION.to_owned()
        ))
    );
}

#[rstest]
fn selected_library_scope_controls_missing_and_unused_diagnostics(
    scenario_builder: ScenarioBuilder,
) {
    let scenario = scenario_builder.with_single_file_pair(
        "Feature: test\n  Scenario: s\n    Given the domain is empty\n",
        concat!(
            "#[step_library]\n",
            "mod filesystem {\n",
            "    #[given(\"the domain is empty\")]\n",
            "    fn empty() {}\n",
            "}\n",
            "#[scenario(path = \"test.feature\", libraries = [accounts])]\n",
            "fn bind() {}\n",
        ),
    );

    let feature_index = scenario
        .state
        .feature_index(&scenario.feature_path)
        .expect("feature index");
    let feature_diagnostics =
        compute_unimplemented_step_diagnostics(&scenario.state, feature_index);
    let rust_diagnostics = compute_unused_step_diagnostics(&scenario.state, &scenario.rust_path);

    assert_eq!(feature_diagnostics.len(), 1);
    assert_eq!(
        feature_diagnostics
            .first()
            .expect("feature diagnostic")
            .code,
        Some(lsp_types::NumberOrString::String(
            CODE_UNIMPLEMENTED_STEP.to_owned()
        ))
    );
    assert_eq!(rust_diagnostics.len(), 1);
    assert_eq!(
        rust_diagnostics.first().expect("Rust diagnostic").code,
        Some(lsp_types::NumberOrString::String(
            CODE_UNUSED_STEP_DEFINITION.to_owned()
        ))
    );
}

#[rstest]
#[case::implemented_step_no_feature_diagnostic(
    "Feature: test\n  Scenario: s\n    Given a step\n",
    concat!(
        "use rstest_bdd_macros::given;\n\n",
        "#[given(\"a step\")]\n",
        "fn step() {}\n",
    ),
    DiagnosticCheckType::Feature
)]
#[case::used_step_definition_no_rust_diagnostic(
    "Feature: test\n  Scenario: s\n    Given a step\n",
    concat!(
        "use rstest_bdd_macros::given;\n\n",
        "#[given(\"a step\")]\n",
        "fn step() {}\n",
    ),
    DiagnosticCheckType::Rust
)]
#[case::parameterized_pattern_no_diagnostics(
    "Feature: test\n  Scenario: s\n    Given I have 5 items\n",
    concat!(
        "use rstest_bdd_macros::given;\n\n",
        "#[given(\"I have {n:u32} items\")]\n",
        "fn items() {}\n",
    ),
    DiagnosticCheckType::Both
)]
fn no_diagnostics_scenarios(
    scenario_builder: ScenarioBuilder,
    #[case] feature_content: &str,
    #[case] rust_content: &str,
    #[case] check_type: DiagnosticCheckType,
) {
    let scenario = scenario_builder.with_single_file_pair(feature_content, rust_content);
    match check_type {
        DiagnosticCheckType::Feature => {
            assert_feature_has_no_unimplemented_steps(&scenario.state, &scenario.feature_path);
        }
        DiagnosticCheckType::Rust => {
            assert_rust_has_no_unused_steps(&scenario.state, &scenario.rust_path);
        }
        DiagnosticCheckType::Both => {
            assert_feature_has_no_unimplemented_steps(&scenario.state, &scenario.feature_path);
            assert_rust_has_no_unused_steps(&scenario.state, &scenario.rust_path);
        }
    }
}

#[rstest]
fn keyword_matching_is_enforced(scenario_builder: ScenarioBuilder) {
    // Given step should not match When implementation
    let scenario = scenario_builder.with_single_file_pair(
        "Feature: test\n  Scenario: s\n    Given a step\n",
        concat!(
            "use rstest_bdd_macros::when;\n\n",
            "#[when(\"a step\")]\n",
            "fn step() {}\n",
        ),
    );

    // Feature step should be unimplemented (Given != When)
    let feature_diags =
        compute_feature_diagnostics_for_path(&scenario.state, &scenario.feature_path);
    assert_eq!(feature_diags.len(), 1, "keyword mismatch should be caught");

    // Rust step should be unused (When != Given)
    let rust_diags = compute_unused_step_diagnostics(&scenario.state, &scenario.rust_path);
    assert_eq!(rust_diags.len(), 1, "When step should be unused");
}

#[rstest]
#[case::given(gherkin::StepType::Given, "given")]
#[case::when(gherkin::StepType::When, "when")]
#[case::then(gherkin::StepType::Then, "then")]
fn step_type_to_attribute_returns_correct_names(
    #[case] step_type: gherkin::StepType,
    #[case] expected: &str,
) {
    assert_eq!(step_type_to_attribute(step_type), expected);
}

// ========================================================================
// Placeholder count mismatch tests
// ========================================================================
