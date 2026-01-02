//! Behavioural tests for on-save diagnostics.
//!
//! These tests verify that diagnostics are correctly computed for unimplemented
//! feature steps and unused step definitions. Diagnostics are triggered on
//! file save and published via the LSP protocol.
//!
//! Note: These tests verify the diagnostic computation logic rather than the
//! actual LSP notification publishing, as that requires a full client socket.

use lsp_types::{DiagnosticSeverity, DidSaveTextDocumentParams, TextDocumentIdentifier, Url};
use rstest_bdd_server::config::ServerConfig;
use rstest_bdd_server::handlers::handle_did_save_text_document;
use rstest_bdd_server::server::ServerState;
use tempfile::TempDir;

#[expect(clippy::expect_used, reason = "test helper uses expect for clarity")]
fn index_file(state: &mut ServerState, path: &std::path::Path) {
    let uri = Url::from_file_path(path).expect("file URI");
    let params = DidSaveTextDocumentParams {
        text_document: TextDocumentIdentifier { uri },
        text: None,
    };
    handle_did_save_text_document(state, params);
}

/// Builder for test scenarios involving diagnostics.
struct DiagnosticsTestScenario {
    dir: TempDir,
    feature_files: Vec<(String, String)>,
    rust_files: Vec<(String, String)>,
    state: ServerState,
}

#[expect(clippy::expect_used, reason = "test builder uses expect for clarity")]
impl DiagnosticsTestScenario {
    fn new() -> Self {
        Self {
            dir: TempDir::new().expect("temp dir"),
            feature_files: Vec::new(),
            rust_files: Vec::new(),
            state: ServerState::new(ServerConfig::default()),
        }
    }

    fn with_feature(mut self, filename: &str, content: &str) -> Self {
        self.feature_files
            .push((filename.to_owned(), content.to_owned()));
        self
    }

    fn with_rust_steps(mut self, filename: &str, content: &str) -> Self {
        self.rust_files
            .push((filename.to_owned(), content.to_owned()));
        self
    }

    fn build(mut self) -> (TempDir, ServerState) {
        // Write and index feature files first
        for (filename, content) in &self.feature_files {
            let path = self.dir.path().join(filename);
            std::fs::write(&path, content).expect("write feature file");
            index_file(&mut self.state, &path);
        }
        // Write and index Rust files
        for (filename, content) in &self.rust_files {
            let path = self.dir.path().join(filename);
            std::fs::write(&path, content).expect("write rust file");
            index_file(&mut self.state, &path);
        }
        (self.dir, self.state)
    }
}

/// Helper to compute unimplemented step diagnostics for a feature file.
#[expect(clippy::expect_used, reason = "test helper uses expect for clarity")]
fn compute_feature_diagnostics(
    state: &ServerState,
    dir: &TempDir,
    filename: &str,
) -> Vec<lsp_types::Diagnostic> {
    let path = dir.path().join(filename);
    let feature_index = state.feature_index(&path).expect("feature index");

    // Reuse the same logic as the diagnostics module
    feature_index
        .steps
        .iter()
        .filter(|step| {
            !state
                .step_registry()
                .steps_for_keyword(step.step_type)
                .iter()
                .any(|compiled| compiled.regex.is_match(&step.text))
        })
        .map(|step| lsp_types::Diagnostic {
            range: lsp_types::Range::default(),
            severity: Some(DiagnosticSeverity::WARNING),
            message: format!(
                "No Rust implementation found for {} step: \"{}\"",
                step.keyword, step.text
            ),
            ..Default::default()
        })
        .collect()
}

/// Helper to compute unused step definition diagnostics for a Rust file.
fn compute_rust_diagnostics(
    state: &ServerState,
    dir: &TempDir,
    filename: &str,
) -> Vec<lsp_types::Diagnostic> {
    let path = dir.path().join(filename);

    state
        .step_registry()
        .steps_for_file(&path)
        .iter()
        .filter(|step_def| {
            !state.all_feature_indices().any(|feature_index| {
                feature_index
                    .steps
                    .iter()
                    .filter(|step| step.step_type == step_def.keyword)
                    .any(|step| step_def.regex.is_match(&step.text))
            })
        })
        .map(|step_def| lsp_types::Diagnostic {
            range: lsp_types::Range::default(),
            severity: Some(DiagnosticSeverity::WARNING),
            message: format!(
                "Step definition is not used by any feature file: #[{}(\"{}\")]",
                match step_def.keyword {
                    gherkin::StepType::Given => "given",
                    gherkin::StepType::When => "when",
                    gherkin::StepType::Then => "then",
                },
                step_def.pattern
            ),
            ..Default::default()
        })
        .collect()
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

#[test]
fn feature_with_unimplemented_steps_reports_diagnostics() {
    let (dir, state) = DiagnosticsTestScenario::new()
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

    let diagnostics = compute_feature_diagnostics(&state, &dir, "test.feature");

    assert_single_diagnostic_contains(&diagnostics, "an unimplemented step");
}

#[test]
fn feature_with_all_steps_implemented_reports_no_diagnostics() {
    let (dir, state) = DiagnosticsTestScenario::new()
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

    let diagnostics = compute_feature_diagnostics(&state, &dir, "test.feature");

    assert!(diagnostics.is_empty());
}

#[test]
fn rust_file_with_unused_definitions_reports_diagnostics() {
    let (dir, state) = DiagnosticsTestScenario::new()
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

    let diagnostics = compute_rust_diagnostics(&state, &dir, "steps.rs");

    assert_single_diagnostic_contains(&diagnostics, "unused step");
}

#[test]
fn rust_file_with_all_definitions_used_reports_no_diagnostics() {
    let (dir, state) = DiagnosticsTestScenario::new()
        .with_feature(
            "test.feature",
            concat!("Feature: test\n", "  Scenario: s\n", "    Given a step\n",),
        )
        .with_rust_steps(
            "steps.rs",
            concat!(
                "use rstest_bdd_macros::given;\n\n",
                "#[given(\"a step\")]\n",
                "fn step() {}\n",
            ),
        )
        .build();

    let diagnostics = compute_rust_diagnostics(&state, &dir, "steps.rs");

    assert!(diagnostics.is_empty());
}

#[test]
fn parameterized_step_matches_feature_step() {
    let (dir, state) = DiagnosticsTestScenario::new()
        .with_feature(
            "test.feature",
            concat!(
                "Feature: test\n",
                "  Scenario: s\n",
                "    Given I have 42 items\n",
            ),
        )
        .with_rust_steps(
            "steps.rs",
            concat!(
                "use rstest_bdd_macros::given;\n\n",
                "#[given(\"I have {n:u32} items\")]\n",
                "fn items() {}\n",
            ),
        )
        .build();

    let feature_diags = compute_feature_diagnostics(&state, &dir, "test.feature");
    let rust_diags = compute_rust_diagnostics(&state, &dir, "steps.rs");

    assert!(feature_diags.is_empty(), "parameterized step should match");
    assert!(rust_diags.is_empty(), "step definition should be used");
}

#[test]
fn keyword_mismatch_produces_diagnostics() {
    // Given step should not match When implementation
    let (dir, state) = DiagnosticsTestScenario::new()
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

#[test]
fn multiple_feature_files_are_checked() {
    let (dir, state) = DiagnosticsTestScenario::new()
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

#[test]
fn step_used_in_any_feature_is_not_unused() {
    let (dir, state) = DiagnosticsTestScenario::new()
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
