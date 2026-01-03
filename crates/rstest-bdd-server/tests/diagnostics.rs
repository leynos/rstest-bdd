//! Behavioural tests for on-save diagnostics.
//!
//! These tests verify that diagnostics are correctly computed for unimplemented
//! feature steps and unused step definitions. Diagnostics are triggered on
//! file save and published via the LSP protocol.
//!
//! Note: These tests verify the diagnostic computation logic rather than the
//! actual LSP notification publishing, as that requires a full client socket.

use lsp_types::{DidSaveTextDocumentParams, TextDocumentIdentifier, Url};
use rstest::{fixture, rstest};
use rstest_bdd_server::config::ServerConfig;
use rstest_bdd_server::handlers::{
    compute_unimplemented_step_diagnostics, compute_unused_step_diagnostics,
    handle_did_save_text_document,
};
use rstest_bdd_server::server::ServerState;
use tempfile::TempDir;

/// Newtype for test file names to improve type safety.
#[derive(Debug, Clone)]
struct Filename(String);

impl From<&str> for Filename {
    fn from(s: &str) -> Self {
        Self(s.to_owned())
    }
}

impl AsRef<str> for Filename {
    fn as_ref(&self) -> &str {
        &self.0
    }
}

/// Newtype for file contents to improve type safety.
#[derive(Debug, Clone)]
struct FileContent(String);

impl From<&str> for FileContent {
    fn from(s: &str) -> Self {
        Self(s.to_owned())
    }
}

impl AsRef<str> for FileContent {
    fn as_ref(&self) -> &str {
        &self.0
    }
}

/// Builder for test scenarios involving diagnostics.
struct ScenarioBuilder {
    dir: TempDir,
    feature_files: Vec<(String, String)>,
    rust_files: Vec<(String, String)>,
    state: ServerState,
}

/// Index a file by simulating a save event.
#[expect(clippy::expect_used, reason = "test helper uses expect for clarity")]
fn index_file(state: &mut ServerState, path: &std::path::Path) {
    let uri = Url::from_file_path(path).expect("file URI");
    let params = DidSaveTextDocumentParams {
        text_document: TextDocumentIdentifier { uri },
        text: None,
    };
    handle_did_save_text_document(state, params);
}

#[expect(clippy::expect_used, reason = "test builder uses expect for clarity")]
impl ScenarioBuilder {
    fn with_feature(
        mut self,
        filename: impl Into<Filename>,
        content: impl Into<FileContent>,
    ) -> Self {
        self.feature_files
            .push((filename.into().0, content.into().0));
        self
    }

    fn with_rust_steps(
        mut self,
        filename: impl Into<Filename>,
        content: impl Into<FileContent>,
    ) -> Self {
        self.rust_files.push((filename.into().0, content.into().0));
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

/// Fixture providing a fresh scenario builder for each test.
#[fixture]
fn scenario_builder() -> ScenarioBuilder {
    #[expect(clippy::expect_used, reason = "fixture panics on temp dir failure")]
    let dir = TempDir::new().expect("temp dir");
    ScenarioBuilder {
        dir,
        feature_files: Vec::new(),
        rust_files: Vec::new(),
        state: ServerState::new(ServerConfig::default()),
    }
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
    let (dir, state) = scenario_builder
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
    let (dir, state) = scenario_builder
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
    let (dir, state) = scenario_builder
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
    "rust"
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
    "both"
)]
fn no_diagnostics_reported(
    scenario_builder: ScenarioBuilder,
    #[case] feature_filename: &str,
    #[case] feature_content: &str,
    #[case] rust_filename: &str,
    #[case] rust_content: &str,
    #[case] check_type: &str,
) {
    let (dir, state) = scenario_builder
        .with_feature(feature_filename, feature_content)
        .with_rust_steps(rust_filename, rust_content)
        .build();

    match check_type {
        "rust" => assert_rust_has_no_diagnostics(&state, &dir, rust_filename),
        "feature" => assert_feature_has_no_diagnostics(&state, &dir, feature_filename),
        "both" => {
            assert_feature_has_no_diagnostics(&state, &dir, feature_filename);
            assert_rust_has_no_diagnostics(&state, &dir, rust_filename);
        }
        _ => panic!("invalid check_type: {check_type}"),
    }
}

#[rstest]
fn keyword_mismatch_produces_diagnostics(scenario_builder: ScenarioBuilder) {
    // Given step should not match When implementation
    let (dir, state) = scenario_builder
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
    let (dir, state) = scenario_builder
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
    let (dir, state) = scenario_builder
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
