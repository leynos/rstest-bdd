//! Behavioural tests for `textDocument/implementation` navigation.
//!
//! These tests verify end-to-end navigation from feature steps in `.feature`
//! files to matching Rust step implementations.

use lsp_types::request::{GotoImplementationParams, GotoImplementationResponse};
use lsp_types::{
    DidSaveTextDocumentParams, PartialResultParams, Position, TextDocumentIdentifier,
    TextDocumentPositionParams, Url, WorkDoneProgressParams,
};
use rstest_bdd_server::config::ServerConfig;
use rstest_bdd_server::handlers::{handle_did_save_text_document, handle_implementation};
use rstest_bdd_server::server::ServerState;
use tempfile::TempDir;

/// Helper to create `GotoImplementationParams` for a given URI and position.
fn make_params(uri: Url, line: u32, character: u32) -> GotoImplementationParams {
    GotoImplementationParams {
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

/// Builder for test scenarios involving implementation navigation.
///
/// This struct provides a fluent API for setting up test fixtures with feature
/// files and Rust step definitions, reducing boilerplate across tests.
struct ImplementationTestScenario {
    dir: TempDir,
    feature_file: Option<(String, String)>,
    rust_files: Vec<(String, String)>,
    state: ServerState,
}

impl ImplementationTestScenario {
    /// Creates a new test scenario with an empty setup.
    #[expect(
        clippy::expect_used,
        reason = "behavioural tests use explicit panics for clarity"
    )]
    fn new() -> Self {
        Self {
            dir: TempDir::new().expect("temp dir"),
            feature_file: None,
            rust_files: Vec::new(),
            state: ServerState::new(ServerConfig::default()),
        }
    }

    /// Sets the feature file to be created during build.
    fn with_feature(mut self, filename: &str, content: &str) -> Self {
        self.feature_file = Some((filename.to_owned(), content.to_owned()));
        self
    }

    /// Adds a Rust step definitions file.
    fn with_rust_steps(mut self, filename: &str, content: &str) -> Self {
        self.rust_files
            .push((filename.to_owned(), content.to_owned()));
        self
    }

    /// Builds the test scenario by creating all files and indexing them.
    ///
    /// Returns a tuple of `(TempDir, PathBuf, ServerState)` where `PathBuf` is
    /// the path to the feature file.
    #[expect(
        clippy::expect_used,
        reason = "behavioural tests use explicit panics for clarity"
    )]
    fn build(mut self) -> (TempDir, std::path::PathBuf, ServerState) {
        // Create and index Rust files first (so registry is populated)
        for (filename, content) in &self.rust_files {
            let path = self.dir.path().join(filename);
            std::fs::write(&path, content).expect("write rust file");
            index_file(&mut self.state, &path);
        }

        // Create and index feature file
        let (filename, content) = self.feature_file.expect("feature file required");
        let feature_path = self.dir.path().join(filename);
        std::fs::write(&feature_path, &content).expect("write feature file");
        index_file(&mut self.state, &feature_path);

        (self.dir, feature_path, self.state)
    }
}

/// Requests implementation locations for a position in a feature file.
///
/// Returns `None` if the handler returns `None`, or extracts the locations
/// array from the response. Panics if the response is not an array variant.
#[expect(
    clippy::expect_used,
    reason = "behavioural tests use explicit panics for clarity"
)]
fn get_implementation_locations(
    state: &ServerState,
    feature_path: &std::path::Path,
    line: u32,
    character: u32,
) -> Option<Vec<lsp_types::Location>> {
    let feature_uri = Url::from_file_path(feature_path).expect("feature URI");
    let params = make_params(feature_uri, line, character);
    let response = handle_implementation(state, &params).expect("implementation response");

    response.map(|resp| match resp {
        GotoImplementationResponse::Array(locs) => locs,
        other => panic!("expected array response, got {other:?}"),
    })
}

#[expect(
    clippy::expect_used,
    reason = "behavioural tests use explicit panics for clarity"
)]
#[test]
fn implementation_navigates_from_feature_step_to_rust_function() {
    let (_dir, feature_path, state) = ImplementationTestScenario::new()
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
        .with_rust_steps(
            "steps.rs",
            concat!(
                "use rstest_bdd_macros::given;\n",
                "\n",
                "#[given(\"a user exists\")]\n",
                "fn a_user_exists() {}\n",
            ),
        )
        .build();

    // Request implementation on the "Given a user exists" line (line 2, 0-indexed)
    let locations = get_implementation_locations(&state, &feature_path, 2, 4)
        .expect("response should have locations");

    assert_eq!(locations.len(), 1, "expected one matching implementation");
    let loc = locations.first().expect("at least one location");
    assert!(
        loc.uri.path().ends_with("steps.rs"),
        "location should be in Rust file"
    );
    assert_eq!(loc.range.start.line, 3, "function is on line 3 (0-indexed)");
}

#[expect(
    clippy::expect_used,
    reason = "behavioural tests use explicit panics for clarity"
)]
#[test]
fn implementation_returns_multiple_locations_for_duplicate_implementations() {
    let (_dir, feature_path, state) = ImplementationTestScenario::new()
        .with_feature(
            "test.feature",
            concat!(
                "Feature: test\n",
                "  Scenario: example\n",
                "    Given a common step\n",
            ),
        )
        .with_rust_steps(
            "steps1.rs",
            concat!(
                "use rstest_bdd_macros::given;\n",
                "\n",
                "#[given(\"a common step\")]\n",
                "fn common_step_impl1() {}\n",
            ),
        )
        .with_rust_steps(
            "steps2.rs",
            concat!(
                "use rstest_bdd_macros::given;\n",
                "\n",
                "#[given(\"a common step\")]\n",
                "fn common_step_impl2() {}\n",
            ),
        )
        .build();

    let locations = get_implementation_locations(&state, &feature_path, 2, 4)
        .expect("response should have locations");

    assert_eq!(locations.len(), 2, "expected two matching implementations");
}

#[expect(
    clippy::expect_used,
    reason = "behavioural tests use explicit panics for clarity"
)]
#[test]
fn implementation_respects_keyword_matching() {
    let (_dir, feature_path, state) = ImplementationTestScenario::new()
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
        .with_rust_steps(
            "steps.rs",
            concat!(
                "use rstest_bdd_macros::when;\n",
                "\n",
                "#[when(\"a step\")]\n",
                "fn when_step() {}\n",
            ),
        )
        .build();

    // Request on Given step - should not match When implementation
    let given_locations = get_implementation_locations(&state, &feature_path, 2, 4);
    assert!(
        given_locations.is_none(),
        "Given step should not match When implementation"
    );

    // Request on When step - should match
    let when_locations = get_implementation_locations(&state, &feature_path, 3, 4)
        .expect("When step should have implementation");
    assert_eq!(
        when_locations.len(),
        1,
        "expected one matching implementation"
    );

    // Request on Then step - should not match When implementation
    let then_locations = get_implementation_locations(&state, &feature_path, 4, 4);
    assert!(
        then_locations.is_none(),
        "Then step should not match When implementation"
    );
}

#[expect(
    clippy::expect_used,
    reason = "behavioural tests use explicit panics for clarity"
)]
#[test]
fn implementation_matches_parameterized_patterns() {
    let (_dir, feature_path, state) = ImplementationTestScenario::new()
        .with_feature(
            "test.feature",
            concat!(
                "Feature: test\n",
                "  Scenario: example\n",
                "    Given I have 5 items\n",
                "    Given I have 10 items\n",
            ),
        )
        .with_rust_steps(
            "steps.rs",
            concat!(
                "use rstest_bdd_macros::given;\n",
                "\n",
                "#[given(\"I have {count:u32} items\")]\n",
                "fn have_items(count: u32) {}\n",
            ),
        )
        .build();

    // Both parameterized steps should match the same implementation
    let locations1 = get_implementation_locations(&state, &feature_path, 2, 4)
        .expect("first step should have implementation");
    assert_eq!(locations1.len(), 1);

    let locations2 = get_implementation_locations(&state, &feature_path, 3, 4)
        .expect("second step should have implementation");
    assert_eq!(locations2.len(), 1);

    // Both should point to the same function
    assert_eq!(
        locations1.first().expect("loc1").uri,
        locations2.first().expect("loc2").uri
    );
}

#[expect(
    clippy::expect_used,
    reason = "behavioural tests use explicit panics for clarity"
)]
#[test]
fn implementation_returns_none_for_non_feature_file() {
    let dir = TempDir::new().expect("temp dir");

    let rust_path = dir.path().join("steps.rs");
    std::fs::write(&rust_path, "fn main() {}\n").expect("write rust file");

    let state = ServerState::new(ServerConfig::default());

    // Request implementation on a Rust file (not feature)
    let rust_uri = Url::from_file_path(&rust_path).expect("rust URI");
    let params = make_params(rust_uri, 0, 0);

    let response = handle_implementation(&state, &params).expect("implementation response");
    assert!(
        response.is_none(),
        "should return None for non-feature files"
    );
}

#[test]
fn implementation_returns_none_when_no_implementation_exists() {
    let (_dir, feature_path, state) = ImplementationTestScenario::new()
        .with_feature(
            "test.feature",
            concat!(
                "Feature: test\n",
                "  Scenario: example\n",
                "    Given an unimplemented step\n",
            ),
        )
        .with_rust_steps(
            "steps.rs",
            concat!(
                "use rstest_bdd_macros::given;\n",
                "\n",
                "#[given(\"a different step\")]\n",
                "fn different_step() {}\n",
            ),
        )
        .build();

    let locations = get_implementation_locations(&state, &feature_path, 2, 4);
    assert!(
        locations.is_none(),
        "should return None when no matching implementation exists"
    );
}

#[test]
fn implementation_returns_none_when_not_on_step_line() {
    let (_dir, feature_path, state) = ImplementationTestScenario::new()
        .with_feature(
            "test.feature",
            concat!(
                "Feature: test\n",
                "  Scenario: example\n",
                "    Given a step\n",
            ),
        )
        .with_rust_steps(
            "steps.rs",
            concat!(
                "use rstest_bdd_macros::given;\n",
                "\n",
                "#[given(\"a step\")]\n",
                "fn a_step() {}\n",
            ),
        )
        .build();

    // Request on Feature line (not a step)
    let locations = get_implementation_locations(&state, &feature_path, 0, 0);
    assert!(
        locations.is_none(),
        "should return None when not on a step line"
    );

    // Request on Scenario line (not a step)
    let locations = get_implementation_locations(&state, &feature_path, 1, 0);
    assert!(
        locations.is_none(),
        "should return None when on scenario line"
    );
}
