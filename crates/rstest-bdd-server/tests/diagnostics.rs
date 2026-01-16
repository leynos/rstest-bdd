//! Behavioural tests for on-save diagnostics.
//!
//! These tests verify that diagnostics are correctly computed for unimplemented
//! feature steps and unused step definitions. Diagnostics are triggered on
//! file save and published via the LSP protocol.
//!
//! Note: These tests verify the diagnostic computation logic rather than the
//! actual LSP notification publishing, as that requires a full client socket.

mod support;

use rstest::{fixture, rstest};
use rstest_bdd_server::handlers::diagnostics::compute::{
    compute_signature_mismatch_diagnostics, compute_table_docstring_mismatch_diagnostics,
};
use rstest_bdd_server::handlers::{
    compute_unimplemented_step_diagnostics, compute_unused_step_diagnostics,
};
use rstest_bdd_server::server::ServerState;
use support::{DiagnosticCheckType, ScenarioBuilder, TestScenario};
use tempfile::TempDir;

/// Fixture providing a fresh scenario builder for each test.
#[fixture]
fn scenario_builder() -> ScenarioBuilder {
    ScenarioBuilder::new()
}

/// Helper to compute unimplemented step diagnostics for a feature file.
#[expect(clippy::expect_used, reason = "test helper uses expect for clarity")]
fn compute_feature_diagnostics(
    state: &ServerState,
    dir: &TempDir,
    filename: impl AsRef<str>,
) -> Vec<lsp_types::Diagnostic> {
    let path = dir.path().join(filename.as_ref());
    let feature_index = state.feature_index(&path).expect("feature index");
    compute_unimplemented_step_diagnostics(state, feature_index)
}

/// Helper to compute unused step definition diagnostics for a Rust file.
fn compute_rust_diagnostics(
    state: &ServerState,
    dir: &TempDir,
    filename: impl AsRef<str>,
) -> Vec<lsp_types::Diagnostic> {
    let path = dir.path().join(filename.as_ref());
    compute_unused_step_diagnostics(state, &path)
}

/// Helper to assert a single diagnostic with an expected message substring.
#[expect(clippy::expect_used, reason = "test helper uses expect for clarity")]
fn assert_single_diagnostic_contains(
    diagnostics: &[lsp_types::Diagnostic],
    expected_substring: &str,
) {
    assert_eq!(diagnostics.len(), 1, "expected exactly one diagnostic");
    assert!(
        diagnostics
            .first()
            .expect("one diagnostic")
            .message
            .contains(expected_substring),
        "diagnostic message should contain '{expected_substring}'"
    );
}

/// Helper to assert a feature file has a diagnostic with expected message.
fn assert_feature_has_diagnostic(
    state: &ServerState,
    dir: &TempDir,
    filename: &str,
    message_substring: &str,
) {
    let diagnostics = compute_feature_diagnostics(state, dir, filename);
    assert_single_diagnostic_contains(&diagnostics, message_substring);
}

/// Helper to assert a rust file has a diagnostic with expected message.
fn assert_rust_has_diagnostic(
    state: &ServerState,
    dir: &TempDir,
    filename: &str,
    message_substring: &str,
) {
    let diagnostics = compute_rust_diagnostics(state, dir, filename);
    assert_single_diagnostic_contains(&diagnostics, message_substring);
}

/// Helper to assert a feature file has no diagnostics.
fn assert_feature_has_no_diagnostics(state: &ServerState, dir: &TempDir, filename: &str) {
    let diagnostics = compute_feature_diagnostics(state, dir, filename);
    assert!(
        diagnostics.is_empty(),
        "expected no diagnostics, found {}",
        diagnostics.len()
    );
}

/// Helper to assert a rust file has no diagnostics.
fn assert_rust_has_no_diagnostics(state: &ServerState, dir: &TempDir, filename: &str) {
    let diagnostics = compute_rust_diagnostics(state, dir, filename);
    assert!(
        diagnostics.is_empty(),
        "expected no diagnostics, found {}",
        diagnostics.len()
    );
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

// ============================================================================
// Placeholder count mismatch tests
// ============================================================================

/// Helper to compute placeholder mismatch diagnostics for a Rust file.
fn compute_placeholder_diagnostics(
    state: &ServerState,
    dir: &TempDir,
    filename: impl AsRef<str>,
) -> Vec<lsp_types::Diagnostic> {
    let path = dir.path().join(filename.as_ref());
    compute_signature_mismatch_diagnostics(state, &path)
}

#[rstest]
#[case::missing_parameter(
    concat!(
        "Feature: test\n",
        "  Scenario: s\n",
        "    Given I have 5 apples\n",
    ),
    concat!(
        "use rstest_bdd_macros::given;\n\n",
        "#[given(\"I have {count} apples\")]\n",
        "fn have_apples() {}\n",
    ),
    1,
    Some("1 placeholder"),
)]
#[case::extra_placeholder(
    concat!(
        "Feature: test\n",
        "  Scenario: s\n",
        "    Given I have 5 red apples\n",
    ),
    concat!(
        "use rstest_bdd_macros::given;\n\n",
        "#[given(\"I have {count} {color} apples\")]\n",
        "fn have_apples(count: u32) {}\n",
    ),
    1,
    Some("2 placeholder"),
)]
#[case::correct_signature(
    concat!(
        "Feature: test\n",
        "  Scenario: s\n",
        "    Given I have 5 apples\n",
    ),
    concat!(
        "use rstest_bdd_macros::given;\n\n",
        "#[given(\"I have {count} apples\")]\n",
        "fn have_apples(count: u32) {}\n",
    ),
    0,
    None,
)]
fn placeholder_count_validation(
    scenario_builder: ScenarioBuilder,
    #[case] feature_content: &str,
    #[case] rust_content: &str,
    #[case] expected_count: usize,
    #[case] message_substring: Option<&str>,
) {
    let TestScenario { dir, state } = scenario_builder
        .with_feature("test.feature", feature_content)
        .with_rust_steps("steps.rs", rust_content)
        .build();

    let diagnostics = compute_placeholder_diagnostics(&state, &dir, "steps.rs");

    assert_eq!(
        diagnostics.len(),
        expected_count,
        "expected {expected_count} diagnostic(s)"
    );
    if let Some(substring) = message_substring {
        #[expect(clippy::expect_used, reason = "checked count > 0 in conditional")]
        let diag = diagnostics.first().expect("checked count > 0");
        assert!(
            diag.message.contains(substring),
            "diagnostic message should contain '{substring}'"
        );
    }
}

// ============================================================================
// Table/docstring expectation mismatch tests
// ============================================================================

/// Helper to compute table/docstring mismatch diagnostics for a feature file.
#[expect(clippy::expect_used, reason = "test helper uses expect for clarity")]
fn compute_table_docstring_diagnostics(
    state: &ServerState,
    dir: &TempDir,
    filename: impl AsRef<str>,
) -> Vec<lsp_types::Diagnostic> {
    let path = dir.path().join(filename.as_ref());
    let feature_index = state.feature_index(&path).expect("feature index");
    compute_table_docstring_mismatch_diagnostics(state, feature_index)
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
    1,
    Some("does not expect"),
)]
#[case::table_expected(
    // Rust expects table, feature doesn't have one
    concat!("Feature: test\n", "  Scenario: s\n", "    Given a step\n"),
    concat!(
        "use rstest_bdd_macros::given;\n",
        "use rstest_bdd::DataTable;\n\n",
        "#[given(\"a step\")]\n",
        "fn a_step(datatable: DataTable) {}\n",
    ),
    1,
    Some("expects a data table"),
)]
#[case::docstring_not_expected(
    // Feature has docstring, Rust doesn't expect it
    concat!(
        "Feature: test\n",
        "  Scenario: s\n",
        "    Given a step\n",
        "      \"\"\"\n",
        "      content\n",
        "      \"\"\"\n",
    ),
    concat!(
        "use rstest_bdd_macros::given;\n\n",
        "#[given(\"a step\")]\n",
        "fn a_step() {}\n",
    ),
    1,
    Some("does not expect"),
)]
#[case::docstring_expected(
    // Rust expects docstring, feature doesn't have one
    concat!("Feature: test\n", "  Scenario: s\n", "    Given a step\n"),
    concat!(
        "use rstest_bdd_macros::given;\n\n",
        "#[given(\"a step\")]\n",
        "fn a_step(docstring: String) {}\n",
    ),
    1,
    Some("expects a doc string"),
)]
#[case::matched_table(
    // Both feature and Rust have table - no diagnostic
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
    0,
    None,
)]
#[case::matched_docstring(
    // Both feature and Rust have docstring - no diagnostic
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
        "fn a_step(docstring: String) {}\n",
    ),
    0,
    None,
)]
fn table_docstring_validation(
    scenario_builder: ScenarioBuilder,
    #[case] feature_content: &str,
    #[case] rust_content: &str,
    #[case] expected_count: usize,
    #[case] message_substring: Option<&str>,
) {
    let TestScenario { dir, state } = scenario_builder
        .with_feature("test.feature", feature_content)
        .with_rust_steps("steps.rs", rust_content)
        .build();

    let diagnostics = compute_table_docstring_diagnostics(&state, &dir, "test.feature");

    assert_eq!(
        diagnostics.len(),
        expected_count,
        "expected {expected_count} diagnostic(s)"
    );
    if let Some(substring) = message_substring {
        #[expect(clippy::expect_used, reason = "checked count > 0 in conditional")]
        let diag = diagnostics.first().expect("checked count > 0");
        assert!(
            diag.message.contains(substring),
            "diagnostic message should contain '{substring}'"
        );
    }
}
