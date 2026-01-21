//! Behavioural tests for placeholder count mismatch diagnostics.
//!
//! These tests verify that diagnostics are correctly emitted when a step
//! pattern's placeholder count doesn't match the function's step argument count.

mod support;

use rstest::{fixture, rstest};
use rstest_bdd_server::handlers::compute_signature_mismatch_diagnostics;
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

/// Helper to compute placeholder mismatch diagnostics for a Rust file.
fn compute_placeholder_diagnostics(
    state: &ServerState,
    dir: &TempDir,
    filename: impl AsRef<str>,
) -> Vec<lsp_types::Diagnostic> {
    let path = dir.path().join(filename.as_ref());
    compute_signature_mismatch_diagnostics(state, &path)
}

// ─────────────────────────────────────────────────────────────────────────────
// Tests
// ─────────────────────────────────────────────────────────────────────────────

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
#[case::repeated_placeholder_name(
    // Pattern has {x} twice (2 occurrences), but only 1 distinct name.
    // The macro counts occurrences, so the function needs 2 parameters named `x`.
    // However, Rust syntax doesn't allow duplicate parameter names, so this tests
    // the diagnostic correctly flags the mismatch (2 placeholders, 1 step arg).
    concat!(
        "Feature: test\n",
        "  Scenario: s\n",
        "    Given I compare 5 with 10\n",
    ),
    concat!(
        "use rstest_bdd_macros::given;\n\n",
        "#[given(\"I compare {x} with {x}\")]\n",
        "fn compare(x: u32) {}\n",
    ),
    1,
    Some("2 placeholder"),
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
