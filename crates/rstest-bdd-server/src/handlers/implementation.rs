//! Handler for `textDocument/implementation` requests.
//!
//! This handler supports navigation from a feature step in a `.feature` file
//! to all matching Rust step implementations. When the cursor is on a step
//! line in a feature file, the handler returns the locations of all Rust
//! functions that implement that step.

use std::sync::Arc;

use async_lsp::ResponseError;
use lsp_types::{
    Location,
    Position,
    Range,
    Url,
    request::{GotoImplementationParams, GotoImplementationResponse},
};
use tracing::debug;

use super::util::{has_extension, lsp_position_to_byte_offset};
use crate::{
    indexing::{CompiledStepDefinition, FeatureFileIndex, IndexedStep},
    server::ServerState,
};

/// Handle `textDocument/implementation` requests.
///
/// When the cursor is on a feature step, returns all matching Rust
/// implementation locations. Returns `None` when the cursor is not on a step
/// or no implementations exist.
///
/// # Errors
///
/// Returns an error if the request parameters are invalid.
pub fn handle_implementation(
    state: &ServerState,
    params: &GotoImplementationParams,
) -> Result<Option<GotoImplementationResponse>, ResponseError> {
    let uri = &params.text_document_position_params.text_document.uri;
    let position = params.text_document_position_params.position;

    // Only handle feature files
    let Ok(path) = uri.to_file_path() else {
        debug!(%uri, "ignoring implementation request for non-file URI");
        return Ok(None);
    };

    if !has_extension(&path, "feature") {
        debug!(
            ?path,
            "ignoring implementation request for non-feature file"
        );
        return Ok(None);
    }

    // Get the feature index for this file (includes the cached source text)
    let Some(feature_index) = state.feature_index(&path) else {
        debug!(?path, "no feature index found for file");
        return Ok(None);
    };

    // Find the step at cursor position using the cached source
    let Some(step) = find_step_at_position(feature_index, &feature_index.source, position) else {
        debug!(?position, "no step at cursor position");
        return Ok(None);
    };

    // Find all matching Rust implementations
    let locations = find_matching_rust_locations(state, feature_index, step);

    if locations.is_empty() {
        debug!(step_text = %step.text, "no matching Rust implementations found");
        return Ok(None);
    }

    debug!(
        step_text = %step.text,
        count = locations.len(),
        "found matching Rust implementations"
    );

    Ok(Some(GotoImplementationResponse::Array(locations)))
}

/// Find the step at the given cursor position.
///
/// Converts the LSP position to a byte offset and finds the step whose span
/// contains that offset.
fn find_step_at_position<'a>(
    index: &'a FeatureFileIndex,
    source: &str,
    position: Position,
) -> Option<&'a IndexedStep> {
    let byte_offset = lsp_position_to_byte_offset(source, position);

    index
        .steps
        .iter()
        .find(|step| step.span.start <= byte_offset && byte_offset < step.span.end)
}

/// Find all Rust implementation locations that match the given feature step.
///
/// Matches are determined by:
/// 1. Step type (Given/When/Then) must match
/// 2. The step text must match the compiled regex pattern
fn find_matching_rust_locations(
    state: &ServerState,
    feature_index: &FeatureFileIndex,
    step: &IndexedStep,
) -> Vec<Location> {
    state
        .steps_for_feature_keyword(&feature_index.path, step.step_type)
        .into_iter()
        .filter(|compiled| compiled.regex.is_match(&step.text))
        .filter_map(build_rust_location)
        .collect()
}

/// Build an LSP Location for a Rust step definition.
///
/// The location points to the function signature line in the Rust source file.
/// The range covers the function line (column 0 to end of next line), which
/// provides a natural jump target for navigation.
fn build_rust_location(step_def: &Arc<CompiledStepDefinition>) -> Option<Location> {
    let uri = Url::from_file_path(&step_def.source_path).ok()?;

    let fn_line = step_def.attribute_span.function_line;
    let range = Range {
        start: Position::new(fn_line, 0),
        end: Position::new(fn_line + 1, 0),
    };

    Some(Location { uri, range })
}

#[cfg(test)]
mod tests {
    //! Unit tests for go-to-implementation handling.

    use std::path::PathBuf;

    use gherkin::Span;
    use lsp_types::{DidSaveTextDocumentParams, TextDocumentIdentifier};
    use tempfile::TempDir;

    use super::*;
    use crate::{
        config::ServerConfig,
        discovery::WorkspaceInfo,
        handlers::handle_did_save_text_document,
    };

    #[test]
    fn find_step_at_position_returns_none_for_empty_index() {
        let source = "Feature: test\n";
        let index = FeatureFileIndex {
            path: PathBuf::from("/tmp/test.feature"),
            source: source.to_owned(),
            steps: Vec::new(),
            example_columns: Vec::new(),
            scenario_outlines: Vec::new(),
        };
        let position = Position::new(0, 0);
        assert!(find_step_at_position(&index, source, position).is_none());
    }

    #[test]
    fn find_step_at_position_returns_step_on_matching_position() {
        use gherkin::StepType;

        use crate::indexing::IndexedStep;

        let source = "Feature: demo\n  Scenario: s\n    Given a step\n";
        let index = FeatureFileIndex {
            path: PathBuf::from("/tmp/test.feature"),
            source: source.to_owned(),
            steps: vec![IndexedStep {
                keyword: "Given".to_owned(),
                step_type: StepType::Given,
                text: "a step".to_owned(),
                span: Span { start: 32, end: 44 },
                docstring: None,
                table: None,
            }],
            example_columns: Vec::new(),
            scenario_outlines: Vec::new(),
        };
        // Position on the step line (line 2, column 4 = "Given")
        let position = Position::new(2, 4);
        let step = find_step_at_position(&index, source, position);
        assert!(step.is_some());
        assert_eq!(step.expect("step").text, "a step");
    }

    #[test]
    fn find_matching_rust_locations_matches_by_keyword_and_pattern() {
        let dir = TempDir::new().expect("temp dir");

        // Create feature file
        let feature_path = dir.path().join("test.feature");
        std::fs::write(
            &feature_path,
            concat!(
                "Feature: test\n",
                "  Scenario: s\n",
                "    Given a step\n",
                "    When a step\n",
            ),
        )
        .expect("write feature file");

        // Create Rust file with Given step
        let rust_path = dir.path().join("steps.rs");
        std::fs::write(
            &rust_path,
            concat!(
                "use rstest_bdd_macros::given;\n",
                "\n",
                "#[given(\"a step\")]\n",
                "fn a_step() {}\n",
            ),
        )
        .expect("write rust file");

        let mut state = ServerState::new(ServerConfig::default());
        state
            .set_workspace_info(WorkspaceInfo {
                root: dir.path().to_path_buf(),
                packages: Vec::new(),
            })
            .expect("configure workspace root");

        // Index both files
        let feature_uri = Url::from_file_path(&feature_path).expect("feature URI");
        handle_did_save_text_document(
            &mut state,
            DidSaveTextDocumentParams {
                text_document: TextDocumentIdentifier { uri: feature_uri },
                text: None,
            },
        );

        let rust_uri = Url::from_file_path(&rust_path).expect("rust URI");
        handle_did_save_text_document(
            &mut state,
            DidSaveTextDocumentParams {
                text_document: TextDocumentIdentifier { uri: rust_uri },
                text: None,
            },
        );

        // Get the indexed feature step
        let feature_index = state.feature_index(&feature_path).expect("feature index");
        let given_step = feature_index
            .steps
            .iter()
            .find(|s| s.step_type == gherkin::StepType::Given)
            .expect("Given step");

        // Find matching locations
        let locations = find_matching_rust_locations(&state, feature_index, given_step);

        // Should find the Rust implementation
        assert_eq!(locations.len(), 1);
        let loc = locations.first().expect("at least one location");
        assert!(loc.uri.path().ends_with("steps.rs"));
        // Range covers the function line (line 3) with a full line range
        assert_eq!(loc.range.start.line, 3);
        assert_eq!(loc.range.start.character, 0);
        assert_eq!(loc.range.end.line, 4);
        assert_eq!(loc.range.end.character, 0);
    }

    #[test]
    fn find_matching_rust_locations_respects_selected_libraries() {
        let dir = TempDir::new().expect("temp dir");
        let feature_path = dir.path().join("test.feature");
        std::fs::write(
            &feature_path,
            "Feature: test\n  Scenario: s\n    Given the domain is empty\n",
        )
        .expect("write feature file");
        let rust_path = dir.path().join("steps.rs");
        std::fs::write(
            &rust_path,
            concat!(
                "#[step_library]\n",
                "mod accounts {\n",
                "    #[given(\"the domain is empty\")]\n",
                "    fn empty() {}\n",
                "}\n",
                "#[step_library]\n",
                "mod filesystem {\n",
                "    #[given(\"the domain is empty\")]\n",
                "    fn empty() {}\n",
                "}\n",
                "#[scenario(path = \"test.feature\", libraries = [accounts])]\n",
                "fn bind() {}\n",
            ),
        )
        .expect("write Rust file");

        let mut state = ServerState::new(ServerConfig::default());
        state
            .set_workspace_info(WorkspaceInfo {
                root: dir.path().to_path_buf(),
                packages: Vec::new(),
            })
            .expect("configure workspace root");
        for path in [&feature_path, &rust_path] {
            let uri = Url::from_file_path(path).expect("file URI");
            handle_did_save_text_document(
                &mut state,
                DidSaveTextDocumentParams {
                    text_document: TextDocumentIdentifier { uri },
                    text: None,
                },
            );
        }

        let feature_index = state.feature_index(&feature_path).expect("feature index");
        let step = feature_index.steps.first().expect("feature step");
        let locations = find_matching_rust_locations(&state, feature_index, step);

        assert_eq!(locations.len(), 1);
        assert_eq!(
            locations
                .first()
                .expect("account implementation")
                .range
                .start
                .line,
            3
        );
    }
}
