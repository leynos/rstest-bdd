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
//! - **Placeholder count mismatches**: Step patterns with a different number
//!   of placeholders than the function has step arguments.
//! - **Table/docstring expectation mismatches**: Feature steps with tables or
//!   docstrings that don't match what the Rust implementation expects.

pub mod compute;
mod publish;

/// Diagnostic source identifier for rstest-bdd diagnostics.
const DIAGNOSTIC_SOURCE: &str = "rstest-bdd";

/// Diagnostic code for unimplemented feature steps.
const CODE_UNIMPLEMENTED_STEP: &str = "unimplemented-step";

/// Diagnostic code for unused step definitions.
const CODE_UNUSED_STEP_DEFINITION: &str = "unused-step-definition";

/// Diagnostic code for placeholder count mismatch in step definitions.
const CODE_PLACEHOLDER_COUNT_MISMATCH: &str = "placeholder-count-mismatch";

/// Diagnostic code for step expecting a data table but feature doesn't provide one.
const CODE_TABLE_EXPECTED: &str = "table-expected";

/// Diagnostic code for feature providing a data table but step doesn't expect one.
const CODE_TABLE_NOT_EXPECTED: &str = "table-not-expected";

/// Diagnostic code for step expecting a docstring but feature doesn't provide one.
const CODE_DOCSTRING_EXPECTED: &str = "docstring-expected";

/// Diagnostic code for feature providing a docstring but step doesn't expect one.
const CODE_DOCSTRING_NOT_EXPECTED: &str = "docstring-not-expected";

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

    /// Asserts exactly one diagnostic exists with the expected code and returns it.
    #[expect(clippy::unwrap_used, reason = "test helper - checked len equals 1")]
    fn assert_single_diagnostic_with_code<'a>(
        diagnostics: &'a [Diagnostic],
        expected_code: &str,
    ) -> &'a Diagnostic {
        assert_eq!(
            diagnostics.len(),
            1,
            "expected exactly 1 diagnostic, found {}",
            diagnostics.len()
        );
        let diag = diagnostics.first().unwrap();
        assert_eq!(
            diag.code,
            Some(lsp_types::NumberOrString::String(expected_code.to_owned())),
            "expected diagnostic code '{expected_code}'"
        );
        diag
    }

    /// Asserts all fragments appear in the diagnostic message.
    fn assert_diagnostic_message_contains(diag: &Diagnostic, fragments: &[&str]) {
        for fragment in fragments {
            assert!(
                diag.message.contains(fragment),
                "diagnostic message should contain '{fragment}', got: {}",
                diag.message
            );
        }
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

    // ========================================================================
    // Placeholder count mismatch tests
    // ========================================================================

    /// Helper to compute signature mismatch diagnostics.
    fn compute_signature_diagnostics_for_path(
        state: &ServerState,
        rust_path: &Path,
    ) -> Vec<Diagnostic> {
        compute::compute_signature_mismatch_diagnostics(state, rust_path)
    }

    #[rstest]
    fn placeholder_count_mismatch_missing_param(scenario_builder: ScenarioBuilder) {
        // Pattern has 1 placeholder, function has 0 step arguments
        let scenario = scenario_builder.with_single_file_pair(
            "Feature: test\n  Scenario: s\n    Given I have 5 apples\n",
            concat!(
                "use rstest_bdd_macros::given;\n\n",
                "#[given(\"I have {count} apples\")]\n",
                "fn have_apples() {}\n",
            ),
        );

        let diagnostics =
            compute_signature_diagnostics_for_path(&scenario.state, &scenario.rust_path);

        let diag =
            assert_single_diagnostic_with_code(&diagnostics, CODE_PLACEHOLDER_COUNT_MISMATCH);
        assert_diagnostic_message_contains(diag, &["1 placeholder", "0 step argument"]);
    }

    #[rstest]
    fn placeholder_count_mismatch_extra_placeholder(scenario_builder: ScenarioBuilder) {
        // Pattern has 2 placeholders, function has 1 step argument
        let scenario = scenario_builder.with_single_file_pair(
            "Feature: test\n  Scenario: s\n    Given I have 5 red apples\n",
            concat!(
                "use rstest_bdd_macros::given;\n\n",
                "#[given(\"I have {count} {color} apples\")]\n",
                "fn have_apples(count: u32) {}\n",
            ),
        );

        let diagnostics =
            compute_signature_diagnostics_for_path(&scenario.state, &scenario.rust_path);

        let diag =
            assert_single_diagnostic_with_code(&diagnostics, CODE_PLACEHOLDER_COUNT_MISMATCH);
        assert_diagnostic_message_contains(diag, &["2 placeholder", "1 step argument"]);
    }

    #[rstest]
    fn no_placeholder_mismatch_when_counts_match(scenario_builder: ScenarioBuilder) {
        // Pattern has 1 placeholder, function has 1 step argument
        let scenario = scenario_builder.with_single_file_pair(
            "Feature: test\n  Scenario: s\n    Given I have 5 apples\n",
            concat!(
                "use rstest_bdd_macros::given;\n\n",
                "#[given(\"I have {count} apples\")]\n",
                "fn have_apples(count: u32) {}\n",
            ),
        );

        let diagnostics =
            compute_signature_diagnostics_for_path(&scenario.state, &scenario.rust_path);

        assert!(
            diagnostics.is_empty(),
            "expected no diagnostics when counts match"
        );
    }

    #[rstest]
    fn fixture_params_excluded_from_step_arg_count(scenario_builder: ScenarioBuilder) {
        // Pattern has 1 placeholder, function has 1 step arg (count) and 1 fixture (context)
        let scenario = scenario_builder.with_single_file_pair(
            "Feature: test\n  Scenario: s\n    Given I have 5 apples\n",
            concat!(
                "use rstest_bdd_macros::given;\n\n",
                "#[given(\"I have {count} apples\")]\n",
                "fn have_apples(count: u32, context: &mut TestContext) {}\n",
            ),
        );

        let diagnostics =
            compute_signature_diagnostics_for_path(&scenario.state, &scenario.rust_path);

        // context is not in placeholder set, so it's a fixture and shouldn't be counted
        assert!(
            diagnostics.is_empty(),
            "fixture params should be excluded from count"
        );
    }

    #[rstest]
    fn datatable_and_docstring_params_excluded(scenario_builder: ScenarioBuilder) {
        // Pattern has 1 placeholder, function has count + datatable + docstring
        let scenario = scenario_builder.with_single_file_pair(
            "Feature: test\n  Scenario: s\n    Given I have 5 apples\n      | col |\n      | val |\n",
            concat!(
                "use rstest_bdd_macros::given;\n",
                "use rstest_bdd::DataTable;\n\n",
                "#[given(\"I have {count} apples\")]\n",
                "fn have_apples(count: u32, datatable: DataTable, docstring: String) {}\n",
            ),
        );

        let diagnostics =
            compute_signature_diagnostics_for_path(&scenario.state, &scenario.rust_path);

        assert!(
            diagnostics.is_empty(),
            "datatable and docstring should be excluded"
        );
    }

    // ========================================================================
    // Table/docstring expectation mismatch tests
    // ========================================================================

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
        compute::compute_table_docstring_mismatch_diagnostics(state, feature_index)
    }

    #[rstest]
    fn table_not_expected_diagnostic(scenario_builder: ScenarioBuilder) {
        // Feature has table, Rust doesn't expect it
        let scenario = scenario_builder.with_single_file_pair(
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
        );

        let diagnostics =
            compute_table_docstring_diagnostics_for_path(&scenario.state, &scenario.feature_path);

        assert_single_diagnostic_with_code(&diagnostics, CODE_TABLE_NOT_EXPECTED);
    }

    #[rstest]
    fn table_expected_diagnostic(scenario_builder: ScenarioBuilder) {
        // Rust expects table, feature doesn't have one
        let scenario = scenario_builder.with_single_file_pair(
            "Feature: test\n  Scenario: s\n    Given a step\n",
            concat!(
                "use rstest_bdd_macros::given;\n",
                "use rstest_bdd::DataTable;\n\n",
                "#[given(\"a step\")]\n",
                "fn a_step(datatable: DataTable) {}\n",
            ),
        );

        let diagnostics =
            compute_table_docstring_diagnostics_for_path(&scenario.state, &scenario.feature_path);

        assert_single_diagnostic_with_code(&diagnostics, CODE_TABLE_EXPECTED);
    }

    #[rstest]
    fn no_table_diagnostic_when_matched(scenario_builder: ScenarioBuilder) {
        // Both have table
        let scenario = scenario_builder.with_single_file_pair(
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
        );

        let diagnostics =
            compute_table_docstring_diagnostics_for_path(&scenario.state, &scenario.feature_path);

        assert!(
            diagnostics.is_empty(),
            "no diagnostic when table expectations match"
        );
    }

    #[rstest]
    fn docstring_not_expected_diagnostic(scenario_builder: ScenarioBuilder) {
        // Feature has docstring, Rust doesn't expect it
        let scenario = scenario_builder.with_single_file_pair(
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
        );

        let diagnostics =
            compute_table_docstring_diagnostics_for_path(&scenario.state, &scenario.feature_path);

        assert_single_diagnostic_with_code(&diagnostics, CODE_DOCSTRING_NOT_EXPECTED);
    }

    #[rstest]
    fn docstring_expected_diagnostic(scenario_builder: ScenarioBuilder) {
        // Rust expects docstring, feature doesn't have one
        let scenario = scenario_builder.with_single_file_pair(
            "Feature: test\n  Scenario: s\n    Given a step\n",
            concat!(
                "use rstest_bdd_macros::given;\n\n",
                "#[given(\"a step\")]\n",
                "fn a_step(docstring: String) {}\n",
            ),
        );

        let diagnostics =
            compute_table_docstring_diagnostics_for_path(&scenario.state, &scenario.feature_path);

        assert_single_diagnostic_with_code(&diagnostics, CODE_DOCSTRING_EXPECTED);
    }
}
