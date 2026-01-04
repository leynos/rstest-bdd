//! Diagnostic computation and publishing for LSP.
//!
//! This module computes diagnostics for consistency issues between feature
//! files and Rust step definitions, publishing them via the LSP protocol.
//! Diagnostics are triggered on file save and report:
//!
//! - **Unimplemented feature steps**: Steps in `.feature` files with no
//!   matching Rust implementation.
//! - **Unused step definitions**: Rust step definitions not matched by any
//!   feature step.

mod compute;
mod publish;

/// Diagnostic source identifier for rstest-bdd diagnostics.
const DIAGNOSTIC_SOURCE: &str = "rstest-bdd";

/// Diagnostic code for unimplemented feature steps.
const CODE_UNIMPLEMENTED_STEP: &str = "unimplemented-step";

/// Diagnostic code for unused step definitions.
const CODE_UNUSED_STEP_DEFINITION: &str = "unused-step-definition";

// Re-export public items
pub use compute::{compute_unimplemented_step_diagnostics, compute_unused_step_diagnostics};
pub use publish::{
    publish_all_feature_diagnostics, publish_feature_diagnostics, publish_rust_diagnostics,
};

#[cfg(test)]
mod tests {
    use super::compute::step_type_to_attribute;
    use super::*;
    use crate::server::ServerState;
    use crate::test_support::{DiagnosticCheckType, ScenarioBuilder};
    use lsp_types::{Diagnostic, DiagnosticSeverity};
    use rstest::{fixture, rstest};
    use std::path::Path;

    /// Fixture providing the infrastructure for diagnostic tests.
    #[fixture]
    fn scenario_builder() -> ScenarioBuilder {
        ScenarioBuilder::new()
    }

    /// Helper to compute feature diagnostics for a path.
    #[expect(
        clippy::expect_used,
        reason = "test helper requires explicit panic for debugging failures"
    )]
    fn compute_feature_diagnostics_for_path(
        state: &ServerState,
        feature_path: &Path,
    ) -> Vec<Diagnostic> {
        let feature_index = state.feature_index(feature_path).expect("feature index");
        compute_unimplemented_step_diagnostics(state, feature_index)
    }

    fn assert_feature_has_no_unimplemented_steps(state: &ServerState, feature_path: &Path) {
        let diags = compute_feature_diagnostics_for_path(state, feature_path);
        assert!(
            diags.is_empty(),
            "expected no unimplemented steps, found {}",
            diags.len()
        );
    }

    fn assert_rust_has_no_unused_steps(state: &ServerState, rust_path: &Path) {
        let diags = compute_unused_step_diagnostics(state, rust_path);
        assert!(
            diags.is_empty(),
            "expected no unused step definitions, found {}",
            diags.len()
        );
    }

    #[rstest]
    #[expect(
        clippy::expect_used,
        reason = "test requires explicit panic for debugging failures"
    )]
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
            .expect("index");
        let diagnostics = compute_unimplemented_step_diagnostics(&scenario.state, feature_index);

        assert_eq!(diagnostics.len(), 1);
        let diag = diagnostics.first().expect("diagnostic");
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
    #[expect(
        clippy::expect_used,
        reason = "test requires explicit panic for debugging failures"
    )]
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

        assert_eq!(diagnostics.len(), 1);
        let diag = diagnostics.first().expect("diagnostic");
        assert!(diag.message.contains("unused step"));
        assert_eq!(
            diag.code,
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
}
