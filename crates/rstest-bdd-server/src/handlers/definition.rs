//! Handler for `textDocument/definition` requests.
//!
//! This handler supports navigation from a Rust step function to matching
//! feature steps in `.feature` files. When the cursor is on a function
//! annotated with `#[given]`, `#[when]`, or `#[then]`, the handler returns
//! all matching feature step locations.

use std::path::Path;
use std::sync::Arc;

use async_lsp::ResponseError;
use lsp_types::{GotoDefinitionParams, GotoDefinitionResponse, Location, Url};
use tracing::debug;

use crate::indexing::CompiledStepDefinition;
use crate::server::ServerState;

use super::util::gherkin_span_to_lsp_range;

/// Handle `textDocument/definition` requests.
///
/// When the cursor is on a Rust step function, returns all matching feature
/// step locations. Returns `None` when the cursor is not on a step function
/// or no matches exist.
///
/// # Errors
///
/// Returns an error if the request parameters are invalid.
pub fn handle_definition(
    state: &ServerState,
    params: &GotoDefinitionParams,
) -> Result<Option<GotoDefinitionResponse>, ResponseError> {
    let uri = &params.text_document_position_params.text_document.uri;
    let position = params.text_document_position_params.position;

    // Only handle Rust files
    let Ok(path) = uri.to_file_path() else {
        debug!(%uri, "ignoring definition request for non-file URI");
        return Ok(None);
    };

    if !is_rust_file(&path) {
        debug!(?path, "ignoring definition request for non-Rust file");
        return Ok(None);
    }

    // Find step definition at cursor position
    let Some(step_def) = find_step_at_position(state, &path, position) else {
        debug!(?position, "no step definition at cursor position");
        return Ok(None);
    };

    // Find all matching feature steps
    let locations = find_matching_feature_locations(state, &step_def);

    if locations.is_empty() {
        debug!(pattern = %step_def.pattern, "no matching feature steps found");
        return Ok(None);
    }

    debug!(
        pattern = %step_def.pattern,
        count = locations.len(),
        "found matching feature steps"
    );

    Ok(Some(GotoDefinitionResponse::Array(locations)))
}

/// Check if a path has a `.rs` extension.
fn is_rust_file(path: &Path) -> bool {
    path.extension()
        .and_then(std::ffi::OsStr::to_str)
        .is_some_and(|ext| ext.eq_ignore_ascii_case("rs"))
}

/// Find the step definition at the given cursor position.
///
/// Uses line-based matching: returns the step whose function definition
/// line matches the cursor's line.
fn find_step_at_position(
    state: &ServerState,
    path: &Path,
    position: lsp_types::Position,
) -> Option<Arc<CompiledStepDefinition>> {
    let steps = state.step_registry().steps_for_file(path);
    let target_line = position.line;

    // Find step whose line matches the cursor position.
    // A step matches if the cursor is on the function definition line or
    // within a reasonable range (attribute line through function line).
    for step in steps {
        // Match if cursor is on the step's function line or the line before
        // (where the attribute typically is)
        if step.line == target_line || step.line == target_line + 1 {
            return Some(Arc::clone(step));
        }
    }

    None
}

/// Find all feature step locations that match the given step definition.
///
/// Matches are determined by:
/// 1. Step type (Given/When/Then) must match
/// 2. The step text must match the compiled regex pattern
fn find_matching_feature_locations(
    state: &ServerState,
    step_def: &CompiledStepDefinition,
) -> Vec<Location> {
    let mut locations = Vec::new();

    for feature_index in state.all_feature_indices() {
        // Read feature file for span conversion
        let source = match std::fs::read_to_string(&feature_index.path) {
            Ok(s) => s,
            Err(err) => {
                debug!(
                    path = %feature_index.path.display(),
                    error = %err,
                    "failed to read feature file for span conversion"
                );
                continue;
            }
        };

        let Ok(uri) = Url::from_file_path(&feature_index.path) else {
            debug!(
                path = %feature_index.path.display(),
                "failed to convert path to URI"
            );
            continue;
        };

        for indexed_step in &feature_index.steps {
            // Match step type (keyword-aware)
            if indexed_step.step_type != step_def.keyword {
                continue;
            }

            // Match using compiled regex
            if step_def.regex.is_match(&indexed_step.text) {
                let range = gherkin_span_to_lsp_range(&source, indexed_step.span);
                locations.push(Location {
                    uri: uri.clone(),
                    range,
                });
            }
        }
    }

    locations
}

#[cfg(test)]
#[expect(
    clippy::expect_used,
    reason = "tests require explicit panic messages for debugging failures"
)]
mod tests {
    use super::*;
    use crate::config::ServerConfig;
    use crate::handlers::handle_did_save_text_document;
    use lsp_types::{DidSaveTextDocumentParams, Position, TextDocumentIdentifier};
    use rstest::{fixture, rstest};
    use std::path::PathBuf;
    use tempfile::TempDir;

    #[fixture]
    fn test_state() -> ServerState {
        ServerState::new(ServerConfig::default())
    }

    #[rstest]
    fn is_rust_file_returns_true_for_rs_extension() {
        assert!(is_rust_file(Path::new("foo.rs")));
        assert!(is_rust_file(Path::new("/path/to/file.rs")));
        assert!(is_rust_file(Path::new("file.RS"))); // case-insensitive
    }

    #[rstest]
    fn is_rust_file_returns_false_for_other_extensions() {
        assert!(!is_rust_file(Path::new("foo.feature")));
        assert!(!is_rust_file(Path::new("foo.txt")));
        assert!(!is_rust_file(Path::new("foo")));
    }

    #[rstest]
    fn find_step_at_position_returns_none_for_empty_registry(test_state: ServerState) {
        let path = PathBuf::from("/tmp/steps.rs");
        let position = Position::new(0, 0);
        assert!(find_step_at_position(&test_state, &path, position).is_none());
    }

    #[rstest]
    fn find_step_at_position_returns_step_on_matching_line() {
        let dir = TempDir::new().expect("temp dir");
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

        let uri = Url::from_file_path(&rust_path).expect("file URI");
        let params = DidSaveTextDocumentParams {
            text_document: TextDocumentIdentifier { uri },
            text: None,
        };

        let mut state = ServerState::new(ServerConfig::default());
        handle_did_save_text_document(&mut state, params);

        // Line 3 (0-indexed) is "fn a_step() {}"
        let position = Position::new(3, 0);
        let step = find_step_at_position(&state, &rust_path, position);
        assert!(step.is_some());
        assert_eq!(step.expect("step").pattern, "a step");
    }

    #[rstest]
    fn find_matching_feature_locations_matches_by_keyword_and_pattern() {
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

        // Get the compiled step
        let steps = state.step_registry().steps_for_file(&rust_path);
        assert_eq!(steps.len(), 1);
        let step_def = steps.first().expect("at least one step");

        // Find matching locations
        let locations = find_matching_feature_locations(&state, step_def);

        // Should match only the Given step, not the When step
        assert_eq!(locations.len(), 1);
        let loc = locations.first().expect("at least one location");
        assert!(loc.uri.path().ends_with("test.feature"));
        assert_eq!(loc.range.start.line, 2); // "Given a step" is on line 2 (0-indexed)
    }
}
