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

#[expect(clippy::expect_used, reason = "test helper uses expect for clarity")]
fn index_file(state: &mut ServerState, path: &std::path::Path) {
    let uri = Url::from_file_path(path).expect("file URI");
    let params = DidSaveTextDocumentParams {
        text_document: TextDocumentIdentifier { uri },
        text: None,
    };
    handle_did_save_text_document(state, params);
}

/// Builder for test scenarios involving implementation navigation.
struct ImplementationTestScenario {
    dir: TempDir,
    feature_file: Option<(String, String)>,
    rust_files: Vec<(String, String)>,
    state: ServerState,
}

#[expect(clippy::expect_used, reason = "test builder uses expect for clarity")]
impl ImplementationTestScenario {
    fn new() -> Self {
        Self {
            dir: TempDir::new().expect("temp dir"),
            feature_file: None,
            rust_files: Vec::new(),
            state: ServerState::new(ServerConfig::default()),
        }
    }

    fn with_feature(mut self, filename: &str, content: &str) -> Self {
        self.feature_file = Some((filename.to_owned(), content.to_owned()));
        self
    }

    fn with_rust_steps(mut self, filename: &str, content: &str) -> Self {
        self.rust_files
            .push((filename.to_owned(), content.to_owned()));
        self
    }

    fn build(mut self) -> (TempDir, std::path::PathBuf, ServerState) {
        for (filename, content) in &self.rust_files {
            let path = self.dir.path().join(filename);
            std::fs::write(&path, content).expect("write rust file");
            index_file(&mut self.state, &path);
        }
        let (filename, content) = self.feature_file.expect("feature file required");
        let feature_path = self.dir.path().join(filename);
        std::fs::write(&feature_path, &content).expect("write feature file");
        index_file(&mut self.state, &feature_path);
        (self.dir, feature_path, self.state)
    }
}

#[expect(clippy::expect_used, reason = "test helper uses expect for clarity")]
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

#[expect(clippy::expect_used, reason = "test uses expect for clarity")]
#[test]
fn implementation_navigates_from_feature_step_to_rust_function() {
    let (_dir, feature_path, state) = ImplementationTestScenario::new()
        .with_feature(
            "test.feature",
            "Feature: test\n  Scenario: example\n    Given a user exists\n",
        )
        .with_rust_steps(
            "steps.rs",
            "use rstest_bdd_macros::given;\n\n#[given(\"a user exists\")]\nfn a_user_exists() {}\n",
        )
        .build();

    let locations = get_implementation_locations(&state, &feature_path, 2, 4)
        .expect("response should have locations");
    assert_eq!(locations.len(), 1, "expected one matching implementation");
    let loc = locations.first().expect("at least one location");
    assert!(
        loc.uri.path().ends_with("steps.rs"),
        "location should be in Rust file"
    );
    assert_eq!(loc.range.start.line, 3, "function is on line 3 (0-indexed)");
    assert_eq!(loc.range.start.character, 0);
    assert_eq!(loc.range.end.line, 4, "range extends to start of next line");
    assert_eq!(loc.range.end.character, 0);
}

#[expect(clippy::expect_used, reason = "test uses expect for clarity")]
#[test]
fn implementation_returns_multiple_locations_for_duplicate_implementations() {
    let (_dir, feature_path, state) = ImplementationTestScenario::new()
        .with_feature(
            "test.feature",
            "Feature: test\n  Scenario: example\n    Given a common step\n",
        )
        .with_rust_steps(
            "steps1.rs",
            "use rstest_bdd_macros::given;\n\n#[given(\"a common step\")]\nfn impl1() {}\n",
        )
        .with_rust_steps(
            "steps2.rs",
            "use rstest_bdd_macros::given;\n\n#[given(\"a common step\")]\nfn impl2() {}\n",
        )
        .build();

    let locations = get_implementation_locations(&state, &feature_path, 2, 4)
        .expect("response should have locations");
    assert_eq!(locations.len(), 2, "expected two matching implementations");
}

#[expect(clippy::expect_used, reason = "test uses expect for clarity")]
#[test]
fn implementation_respects_keyword_matching() {
    let (_dir, feature_path, state) = ImplementationTestScenario::new()
        .with_feature(
            "test.feature",
            "Feature: test\n  Scenario: example\n    Given a step\n    When a step\n    Then a step\n",
        )
        .with_rust_steps(
            "steps.rs",
            "use rstest_bdd_macros::when;\n\n#[when(\"a step\")]\nfn when_step() {}\n",
        )
        .build();

    // Given step should not match When implementation
    assert!(get_implementation_locations(&state, &feature_path, 2, 4).is_none());
    // When step should match
    let when_locs = get_implementation_locations(&state, &feature_path, 3, 4)
        .expect("When step should have implementation");
    assert_eq!(when_locs.len(), 1);
    // Then step should not match When implementation
    assert!(get_implementation_locations(&state, &feature_path, 4, 4).is_none());
}

#[expect(clippy::expect_used, reason = "test uses expect for clarity")]
#[test]
fn implementation_matches_parameterised_patterns() {
    let (_dir, feature_path, state) = ImplementationTestScenario::new()
        .with_feature(
            "test.feature",
            "Feature: test\n  Scenario: example\n    Given I have 5 items\n    Given I have 10 items\n",
        )
        .with_rust_steps(
            "steps.rs",
            "use rstest_bdd_macros::given;\n\n#[given(\"I have {count:u32} items\")]\nfn have_items(count: u32) {}\n",
        )
        .build();

    let loc1 = get_implementation_locations(&state, &feature_path, 2, 4)
        .expect("first step should have implementation");
    let loc2 = get_implementation_locations(&state, &feature_path, 3, 4)
        .expect("second step should have implementation");
    assert_eq!(loc1.len(), 1);
    assert_eq!(loc2.len(), 1);
    assert_eq!(
        loc1.first().expect("loc1").uri,
        loc2.first().expect("loc2").uri
    );
}

#[expect(clippy::expect_used, reason = "test uses expect for clarity")]
#[test]
fn implementation_returns_none_for_non_feature_file() {
    let dir = TempDir::new().expect("temp dir");
    let rust_path = dir.path().join("steps.rs");
    std::fs::write(&rust_path, "fn main() {}\n").expect("write rust file");
    let state = ServerState::new(ServerConfig::default());

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
            "Feature: test\n  Scenario: example\n    Given an unimplemented step\n",
        )
        .with_rust_steps(
            "steps.rs",
            "use rstest_bdd_macros::given;\n\n#[given(\"a different step\")]\nfn different() {}\n",
        )
        .build();

    assert!(get_implementation_locations(&state, &feature_path, 2, 4).is_none());
}

#[test]
fn implementation_returns_none_when_not_on_step_line() {
    let (_dir, feature_path, state) = ImplementationTestScenario::new()
        .with_feature(
            "test.feature",
            "Feature: test\n  Scenario: example\n    Given a step\n",
        )
        .with_rust_steps(
            "steps.rs",
            "use rstest_bdd_macros::given;\n\n#[given(\"a step\")]\nfn a_step() {}\n",
        )
        .build();

    // Request on Feature line (not a step)
    assert!(get_implementation_locations(&state, &feature_path, 0, 0).is_none());
    // Request on Scenario line (not a step)
    assert!(get_implementation_locations(&state, &feature_path, 1, 0).is_none());
}

#[expect(clippy::expect_used, reason = "test uses expect for clarity")]
#[test]
fn implementation_resolves_and_but_keywords_to_preceding_step_type() {
    // And/But keywords inherit their step type from the preceding Given/When/Then step.
    let (_dir, feature_path, state) = ImplementationTestScenario::new()
        .with_feature(
            "test.feature",
            concat!(
                "Feature: And/But keyword resolution\n",
                "  Scenario: example\n",
                "    Given a precondition\n",
                "    And another precondition\n",
                "    But not this precondition\n",
                "    When an action occurs\n",
                "    And another action\n",
                "    Then a result is expected\n",
                "    But not this result\n",
            ),
        )
        .with_rust_steps(
            "steps.rs",
            concat!(
                "use rstest_bdd_macros::{given, when, then};\n\n",
                "#[given(\"another precondition\")]\nfn given_another() {}\n\n",
                "#[given(\"not this precondition\")]\nfn given_but_not() {}\n\n",
                "#[when(\"another action\")]\nfn when_another() {}\n\n",
                "#[then(\"not this result\")]\nfn then_but_not() {}\n",
            ),
        )
        .build();

    // "And another precondition" (line 3) should resolve to Given
    let and_given = get_implementation_locations(&state, &feature_path, 3, 4)
        .expect("And (Given) step should have implementation");
    assert_eq!(
        and_given.len(),
        1,
        "And step should match Given implementation"
    );

    // "But not this precondition" (line 4) should resolve to Given
    let but_given = get_implementation_locations(&state, &feature_path, 4, 4)
        .expect("But (Given) step should have implementation");
    assert_eq!(
        but_given.len(),
        1,
        "But step should match Given implementation"
    );

    // "And another action" (line 6) should resolve to When
    let and_when = get_implementation_locations(&state, &feature_path, 6, 4)
        .expect("And (When) step should have implementation");
    assert_eq!(
        and_when.len(),
        1,
        "And step should match When implementation"
    );

    // "But not this result" (line 8) should resolve to Then
    let but_then = get_implementation_locations(&state, &feature_path, 8, 4)
        .expect("But (Then) step should have implementation");
    assert_eq!(
        but_then.len(),
        1,
        "But step should match Then implementation"
    );
}
