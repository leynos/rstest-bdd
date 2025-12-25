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

#[expect(
    clippy::expect_used,
    reason = "behavioural tests use explicit panics for clarity"
)]
#[test]
fn definition_navigates_from_rust_step_to_matching_feature_step() {
    let dir = TempDir::new().expect("temp dir");

    // Create feature file
    let feature_path = dir.path().join("test.feature");
    std::fs::write(
        &feature_path,
        concat!(
            "Feature: test\n",
            "  Scenario: example\n",
            "    Given a user exists\n",
            "    When the user logs in\n",
            "    Then the user is authenticated\n",
        ),
    )
    .expect("write feature file");

    // Create Rust file with step definitions
    let rust_path = dir.path().join("steps.rs");
    std::fs::write(
        &rust_path,
        concat!(
            "use rstest_bdd_macros::given;\n",
            "\n",
            "#[given(\"a user exists\")]\n",
            "fn a_user_exists() {}\n",
        ),
    )
    .expect("write rust file");

    let mut state = ServerState::new(ServerConfig::default());

    // Index both files
    index_file(&mut state, &feature_path);
    index_file(&mut state, &rust_path);

    // Request definition at the step function line
    let rust_uri = Url::from_file_path(&rust_path).expect("rust URI");
    let params = make_params(rust_uri, 3, 0); // "fn a_user_exists() {}"

    let response = handle_definition(&state, &params).expect("definition response");
    let locations = match response.expect("response should have locations") {
        lsp_types::GotoDefinitionResponse::Array(locs) => locs,
        other => panic!("expected array response, got {other:?}"),
    };

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
    let dir = TempDir::new().expect("temp dir");

    // Create two feature files with the same step
    let feature1_path = dir.path().join("feature1.feature");
    std::fs::write(
        &feature1_path,
        concat!(
            "Feature: first\n",
            "  Scenario: s1\n",
            "    Given a common step\n",
        ),
    )
    .expect("write feature1 file");

    let feature2_path = dir.path().join("feature2.feature");
    std::fs::write(
        &feature2_path,
        concat!(
            "Feature: second\n",
            "  Scenario: s2\n",
            "    Given a common step\n",
        ),
    )
    .expect("write feature2 file");

    // Create Rust file with step definition
    let rust_path = dir.path().join("steps.rs");
    std::fs::write(
        &rust_path,
        concat!(
            "use rstest_bdd_macros::given;\n",
            "\n",
            "#[given(\"a common step\")]\n",
            "fn a_common_step() {}\n",
        ),
    )
    .expect("write rust file");

    let mut state = ServerState::new(ServerConfig::default());

    // Index all files
    index_file(&mut state, &feature1_path);
    index_file(&mut state, &feature2_path);
    index_file(&mut state, &rust_path);

    // Request definition
    let rust_uri = Url::from_file_path(&rust_path).expect("rust URI");
    let params = make_params(rust_uri, 3, 0);

    let response = handle_definition(&state, &params).expect("definition response");
    let locations = match response.expect("response should have locations") {
        lsp_types::GotoDefinitionResponse::Array(locs) => locs,
        other => panic!("expected array response, got {other:?}"),
    };

    assert_eq!(locations.len(), 2, "expected two matching feature steps");
}

#[expect(
    clippy::expect_used,
    reason = "behavioural tests use explicit panics for clarity"
)]
#[test]
fn definition_respects_keyword_matching() {
    let dir = TempDir::new().expect("temp dir");

    // Create feature file with Given and When steps using the same text
    let feature_path = dir.path().join("test.feature");
    std::fs::write(
        &feature_path,
        concat!(
            "Feature: test\n",
            "  Scenario: example\n",
            "    Given a step\n",
            "    When a step\n",
            "    Then a step\n",
        ),
    )
    .expect("write feature file");

    // Create Rust file with only a Given step
    let rust_path = dir.path().join("steps.rs");
    std::fs::write(
        &rust_path,
        concat!(
            "use rstest_bdd_macros::given;\n",
            "\n",
            "#[given(\"a step\")]\n",
            "fn given_step() {}\n",
        ),
    )
    .expect("write rust file");

    let mut state = ServerState::new(ServerConfig::default());

    // Index both files
    index_file(&mut state, &feature_path);
    index_file(&mut state, &rust_path);

    // Request definition
    let rust_uri = Url::from_file_path(&rust_path).expect("rust URI");
    let params = make_params(rust_uri, 3, 0);

    let response = handle_definition(&state, &params).expect("definition response");
    let locations = match response.expect("response should have locations") {
        lsp_types::GotoDefinitionResponse::Array(locs) => locs,
        other => panic!("expected array response, got {other:?}"),
    };

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
    let dir = TempDir::new().expect("temp dir");

    // Create feature file with parameterized step
    let feature_path = dir.path().join("test.feature");
    std::fs::write(
        &feature_path,
        concat!(
            "Feature: test\n",
            "  Scenario: example\n",
            "    Given I have 5 items\n",
            "    Given I have 10 items\n",
        ),
    )
    .expect("write feature file");

    // Create Rust file with parameterized step pattern
    let rust_path = dir.path().join("steps.rs");
    std::fs::write(
        &rust_path,
        concat!(
            "use rstest_bdd_macros::given;\n",
            "\n",
            "#[given(\"I have {count:u32} items\")]\n",
            "fn have_items(count: u32) {}\n",
        ),
    )
    .expect("write rust file");

    let mut state = ServerState::new(ServerConfig::default());

    // Index both files
    index_file(&mut state, &feature_path);
    index_file(&mut state, &rust_path);

    // Request definition
    let rust_uri = Url::from_file_path(&rust_path).expect("rust URI");
    let params = make_params(rust_uri, 3, 0);

    let response = handle_definition(&state, &params).expect("definition response");
    let locations = match response.expect("response should have locations") {
        lsp_types::GotoDefinitionResponse::Array(locs) => locs,
        other => panic!("expected array response, got {other:?}"),
    };

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
