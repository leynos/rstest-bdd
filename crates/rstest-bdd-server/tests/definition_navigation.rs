//! Behavioural tests for `textDocument/definition` navigation.
//!
//! These tests verify end-to-end navigation from Rust step functions to
//! matching feature steps in `.feature` files.

use lsp_types::{
    DidSaveTextDocumentParams, GotoDefinitionParams, PartialResultParams, Position,
    TextDocumentIdentifier, TextDocumentPositionParams, Url, WorkDoneProgressParams,
};
use rstest_bdd_server::config::ServerConfig;
use rstest_bdd_server::handlers::{handle_definition, handle_did_save_text_document};
use rstest_bdd_server::server::ServerState;
use tempfile::TempDir;

/// Helper to create `GotoDefinitionParams` for a given URI and position.
fn make_params(uri: Url, line: u32, character: u32) -> GotoDefinitionParams {
    GotoDefinitionParams {
        text_document_position_params: TextDocumentPositionParams {
            text_document: TextDocumentIdentifier { uri },
            position: Position::new(line, character),
        },
        work_done_progress_params: WorkDoneProgressParams::default(),
        partial_result_params: PartialResultParams::default(),
    }
}

/// Helper function to index a file by simulating a didSave notification.
#[expect(
    clippy::expect_used,
    reason = "behavioural tests use explicit panics for clarity"
)]
fn index_file(state: &mut ServerState, path: &std::path::Path) {
    let uri = Url::from_file_path(path).expect("file URI");
    let params = DidSaveTextDocumentParams {
        text_document: TextDocumentIdentifier { uri },
        text: None,
    };
    handle_did_save_text_document(state, params);
}

/// Builder for test scenarios involving definition navigation.
///
/// This struct provides a fluent API for setting up test fixtures with feature
/// files and Rust step definitions, reducing boilerplate across tests.
struct DefinitionTestScenario {
    dir: TempDir,
    feature_files: Vec<(String, String)>,
    rust_file_content: String,
    state: ServerState,
}

impl DefinitionTestScenario {
    /// Creates a new test scenario with an empty setup.
    #[expect(
        clippy::expect_used,
        reason = "behavioural tests use explicit panics for clarity"
    )]
    fn new() -> Self {
        Self {
            dir: TempDir::new().expect("temp dir"),
            feature_files: Vec::new(),
            rust_file_content: String::new(),
            state: ServerState::new(ServerConfig::default()),
        }
    }

    /// Adds a feature file to be created during build.
    fn with_feature(mut self, filename: &str, content: &str) -> Self {
        self.feature_files
            .push((filename.to_owned(), content.to_owned()));
        self
    }

    /// Sets the Rust step definitions file content.
    fn with_rust_steps(mut self, content: &str) -> Self {
        // Using clone_into() as recommended by Clippy for efficiency
        // (can reuse existing String allocation)
        content.clone_into(&mut self.rust_file_content);
        self
    }

    /// Builds the test scenario by creating all files and indexing them.
    ///
    /// Returns a tuple of `(TempDir, PathBuf, ServerState)` where `PathBuf` is
    /// the path to the Rust steps file.
    #[expect(
        clippy::expect_used,
        reason = "behavioural tests use explicit panics for clarity"
    )]
    fn build(mut self) -> (TempDir, std::path::PathBuf, ServerState) {
        // Create and index feature files
        for (filename, content) in &self.feature_files {
            let path = self.dir.path().join(filename);
            std::fs::write(&path, content).expect("write feature file");
            index_file(&mut self.state, &path);
        }

        // Create and index Rust file
        let rust_path = self.dir.path().join("steps.rs");
        std::fs::write(&rust_path, &self.rust_file_content).expect("write rust file");
        index_file(&mut self.state, &rust_path);

        (self.dir, rust_path, self.state)
    }
}

/// Requests definition locations for a position in a Rust file.
///
/// Returns `None` if the handler returns `None`, or extracts the locations
/// array from the response. Panics if the response is not an array variant.
#[expect(
    clippy::expect_used,
    reason = "behavioural tests use explicit panics for clarity"
)]
fn get_definition_locations(
    state: &ServerState,
    rust_path: &std::path::Path,
    line: u32,
    character: u32,
) -> Option<Vec<lsp_types::Location>> {
    let rust_uri = Url::from_file_path(rust_path).expect("rust URI");
    let params = make_params(rust_uri, line, character);
    let response = handle_definition(state, &params).expect("definition response");

    response.map(|resp| match resp {
        lsp_types::GotoDefinitionResponse::Array(locs) => locs,
        other => panic!("expected array response, got {other:?}"),
    })
}

#[expect(
    clippy::expect_used,
    reason = "behavioural tests use explicit panics for clarity"
)]
#[test]
fn definition_navigates_from_rust_step_to_matching_feature_step() {
    let (_dir, rust_path, state) = DefinitionTestScenario::new()
        .with_feature(
            "test.feature",
            concat!(
                "Feature: test\n",
                "  Scenario: example\n",
                "    Given a user exists\n",
                "    When the user logs in\n",
                "    Then the user is authenticated\n",
            ),
        )
        .with_rust_steps(concat!(
            "use rstest_bdd_macros::given;\n",
            "\n",
            "#[given(\"a user exists\")]\n",
            "fn a_user_exists() {}\n",
        ))
        .build();

    let locations =
        get_definition_locations(&state, &rust_path, 3, 0).expect("response should have locations");

    assert_eq!(locations.len(), 1, "expected one matching feature step");
    let loc = locations.first().expect("at least one location");
    assert!(
        loc.uri.path().ends_with("test.feature"),
        "location should be in feature file"
    );
    assert_eq!(loc.range.start.line, 2, "step is on line 2 (0-indexed)");
}

#[expect(
    clippy::expect_used,
    reason = "behavioural tests use explicit panics for clarity"
)]
#[test]
fn definition_returns_multiple_locations_for_multiple_matches() {
    let (_dir, rust_path, state) = DefinitionTestScenario::new()
        .with_feature(
            "feature1.feature",
            concat!(
                "Feature: first\n",
                "  Scenario: s1\n",
                "    Given a common step\n",
            ),
        )
        .with_feature(
            "feature2.feature",
            concat!(
                "Feature: second\n",
                "  Scenario: s2\n",
                "    Given a common step\n",
            ),
        )
        .with_rust_steps(concat!(
            "use rstest_bdd_macros::given;\n",
            "\n",
            "#[given(\"a common step\")]\n",
            "fn a_common_step() {}\n",
        ))
        .build();

    let locations =
        get_definition_locations(&state, &rust_path, 3, 0).expect("response should have locations");

    assert_eq!(locations.len(), 2, "expected two matching feature steps");
}

#[expect(
    clippy::expect_used,
    reason = "behavioural tests use explicit panics for clarity"
)]
#[test]
fn definition_respects_keyword_matching() {
    let (_dir, rust_path, state) = DefinitionTestScenario::new()
        .with_feature(
            "test.feature",
            concat!(
                "Feature: test\n",
                "  Scenario: example\n",
                "    Given a step\n",
                "    When a step\n",
                "    Then a step\n",
            ),
        )
        .with_rust_steps(concat!(
            "use rstest_bdd_macros::given;\n",
            "\n",
            "#[given(\"a step\")]\n",
            "fn given_step() {}\n",
        ))
        .build();

    let locations =
        get_definition_locations(&state, &rust_path, 3, 0).expect("response should have locations");

    // Should only match the Given step, not the When or Then steps
    assert_eq!(
        locations.len(),
        1,
        "expected only one matching feature step (Given)"
    );
    let loc = locations.first().expect("at least one location");
    assert_eq!(loc.range.start.line, 2, "should match Given on line 2");
}

#[expect(
    clippy::expect_used,
    reason = "behavioural tests use explicit panics for clarity"
)]
#[test]
fn definition_matches_parameterized_patterns() {
    let (_dir, rust_path, state) = DefinitionTestScenario::new()
        .with_feature(
            "test.feature",
            concat!(
                "Feature: test\n",
                "  Scenario: example\n",
                "    Given I have 5 items\n",
                "    Given I have 10 items\n",
            ),
        )
        .with_rust_steps(concat!(
            "use rstest_bdd_macros::given;\n",
            "\n",
            "#[given(\"I have {count:u32} items\")]\n",
            "fn have_items(count: u32) {}\n",
        ))
        .build();

    let locations =
        get_definition_locations(&state, &rust_path, 3, 0).expect("response should have locations");

    // Should match both parameterized steps
    assert_eq!(locations.len(), 2, "expected two matching feature steps");
}

#[expect(
    clippy::expect_used,
    reason = "behavioural tests use explicit panics for clarity"
)]
#[test]
fn definition_returns_none_for_non_rust_file() {
    let dir = TempDir::new().expect("temp dir");

    let feature_path = dir.path().join("test.feature");
    std::fs::write(&feature_path, "Feature: test\n").expect("write feature file");

    let state = ServerState::new(ServerConfig::default());

    // Request definition on a feature file (not Rust)
    let feature_uri = Url::from_file_path(&feature_path).expect("feature URI");
    let params = make_params(feature_uri, 0, 0);

    let response = handle_definition(&state, &params).expect("definition response");
    assert!(response.is_none(), "should return None for non-Rust files");
}

#[expect(
    clippy::expect_used,
    reason = "behavioural tests use explicit panics for clarity"
)]
#[test]
fn definition_returns_none_when_no_step_at_position() {
    let dir = TempDir::new().expect("temp dir");

    // Create Rust file with a step definition
    let rust_path = dir.path().join("steps.rs");
    std::fs::write(
        &rust_path,
        concat!(
            "use rstest_bdd_macros::given;\n",
            "\n",
            "#[given(\"a step\")]\n",
            "fn a_step() {}\n",
            "\n",
            "fn not_a_step() {}\n",
        ),
    )
    .expect("write rust file");

    let mut state = ServerState::new(ServerConfig::default());
    index_file(&mut state, &rust_path);

    // Request definition at a non-step function line
    let rust_uri = Url::from_file_path(&rust_path).expect("rust URI");
    let params = make_params(rust_uri, 5, 0); // "fn not_a_step() {}"

    let response = handle_definition(&state, &params).expect("definition response");
    assert!(
        response.is_none(),
        "should return None when not on a step function"
    );
}
