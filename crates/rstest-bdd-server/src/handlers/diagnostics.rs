//! Diagnostic computation and publishing for LSP.
//!
//! This module computes diagnostics for consistency issues between feature
//! files and Rust step definitions, publishing them via the LSP protocol.
//! Diagnostics are triggered on file save and report:
//!
//! - **Unimplemented feature steps**: Steps in `.feature` files with no
//!   matching Rust implementation.
//! - **Unused step definitions**: Rust step definitions not matched by any
//!   feature step.

use std::{path::Path, sync::Arc};

use async_lsp::lsp_types::notification;
use lsp_types::{Diagnostic, DiagnosticSeverity, PublishDiagnosticsParams, Range, Url};
use tracing::{debug, warn};

use crate::indexing::{CompiledStepDefinition, FeatureFileIndex, IndexedStep};
use crate::server::ServerState;

use super::util::gherkin_span_to_lsp_range;

/// Diagnostic source identifier for rstest-bdd diagnostics.
const DIAGNOSTIC_SOURCE: &str = "rstest-bdd";

/// Diagnostic code for unimplemented feature steps.
const CODE_UNIMPLEMENTED_STEP: &str = "unimplemented-step";

/// Diagnostic code for unused step definitions.
const CODE_UNUSED_STEP_DEFINITION: &str = "unused-step-definition";

/// Publish diagnostics for a single feature file.
///
/// Computes unimplemented step diagnostics and publishes them via the client
/// socket. Publishes an empty array if all steps have implementations,
/// clearing any previous diagnostics.
pub fn publish_feature_diagnostics(state: &ServerState, feature_path: &Path) {
    let Some(client) = state.client() else {
        debug!("no client socket available for publishing diagnostics");
        return;
    };

    let Some(feature_index) = state.feature_index(feature_path) else {
        debug!(path = %feature_path.display(), "no feature index for diagnostics");
        return;
    };

    let Ok(uri) = Url::from_file_path(feature_path) else {
        warn!(path = %feature_path.display(), "cannot convert path to URI");
        return;
    };

    let diagnostics = compute_unimplemented_step_diagnostics(state, feature_index);

    let params = PublishDiagnosticsParams::new(uri, diagnostics, None);
    if let Err(err) = client.notify::<notification::PublishDiagnostics>(params) {
        warn!(error = %err, "failed to publish feature diagnostics");
    }
}

/// Publish diagnostics for all feature files.
///
/// Called when a Rust file is saved, as step definition changes may affect
/// which feature steps are unimplemented.
pub fn publish_all_feature_diagnostics(state: &ServerState) {
    // Collect paths first to avoid borrowing issues
    let feature_paths: Vec<_> = state
        .all_feature_indices()
        .map(|index| index.path.clone())
        .collect();

    for path in feature_paths {
        publish_feature_diagnostics(state, &path);
    }
}

/// Publish diagnostics for unused step definitions in a Rust file.
pub fn publish_rust_diagnostics(state: &ServerState, rust_path: &Path) {
    let Some(client) = state.client() else {
        debug!("no client socket available for publishing diagnostics");
        return;
    };

    let Ok(uri) = Url::from_file_path(rust_path) else {
        warn!(path = %rust_path.display(), "cannot convert path to URI");
        return;
    };

    let diagnostics = compute_unused_step_diagnostics(state, rust_path);

    let params = PublishDiagnosticsParams::new(uri, diagnostics, None);
    if let Err(err) = client.notify::<notification::PublishDiagnostics>(params) {
        warn!(error = %err, "failed to publish rust diagnostics");
    }
}

/// Compute diagnostics for unimplemented feature steps.
///
/// For each step in the feature file, checks if there is at least one matching
/// Rust implementation. Steps without implementations get a warning diagnostic.
#[must_use]
pub fn compute_unimplemented_step_diagnostics(
    state: &ServerState,
    feature_index: &FeatureFileIndex,
) -> Vec<Diagnostic> {
    feature_index
        .steps
        .iter()
        .filter(|step| !has_matching_implementation(state, step))
        .map(|step| build_unimplemented_step_diagnostic(feature_index, step))
        .collect()
}

/// Check if a feature step has at least one matching Rust implementation.
fn has_matching_implementation(state: &ServerState, step: &IndexedStep) -> bool {
    state
        .step_registry()
        .steps_for_keyword(step.step_type)
        .iter()
        .any(|compiled| compiled.regex.is_match(&step.text))
}

/// Build a diagnostic for an unimplemented feature step.
fn build_unimplemented_step_diagnostic(
    feature_index: &FeatureFileIndex,
    step: &IndexedStep,
) -> Diagnostic {
    let range = gherkin_span_to_lsp_range(&feature_index.source, step.span);

    Diagnostic {
        range,
        severity: Some(DiagnosticSeverity::WARNING),
        code: Some(lsp_types::NumberOrString::String(
            CODE_UNIMPLEMENTED_STEP.to_owned(),
        )),
        code_description: None,
        source: Some(DIAGNOSTIC_SOURCE.to_owned()),
        message: format!(
            "No Rust implementation found for {} step: \"{}\"",
            step.keyword, step.text
        ),
        related_information: None,
        tags: None,
        data: None,
    }
}

/// Compute diagnostics for unused step definitions in a Rust file.
///
/// For each step definition in the file, checks if any feature step matches it.
/// Definitions without matches get a warning diagnostic.
#[must_use]
pub fn compute_unused_step_diagnostics(state: &ServerState, rust_path: &Path) -> Vec<Diagnostic> {
    state
        .step_registry()
        .steps_for_file(rust_path)
        .iter()
        .filter(|step_def| !has_matching_feature_step(state, step_def))
        .map(build_unused_step_diagnostic)
        .collect()
}

/// Check if a Rust step definition is matched by at least one feature step.
fn has_matching_feature_step(state: &ServerState, step_def: &Arc<CompiledStepDefinition>) -> bool {
    state.all_feature_indices().any(|feature_index| {
        feature_index
            .steps
            .iter()
            .filter(|step| step.step_type == step_def.keyword)
            .any(|step| step_def.regex.is_match(&step.text))
    })
}

/// Build a diagnostic for an unused step definition.
fn build_unused_step_diagnostic(step_def: &Arc<CompiledStepDefinition>) -> Diagnostic {
    // Range spans the function definition line.
    let range = Range {
        start: lsp_types::Position::new(step_def.line, 0),
        end: lsp_types::Position::new(step_def.line + 1, 0),
    };

    Diagnostic {
        range,
        severity: Some(DiagnosticSeverity::WARNING),
        code: Some(lsp_types::NumberOrString::String(
            CODE_UNUSED_STEP_DEFINITION.to_owned(),
        )),
        code_description: None,
        source: Some(DIAGNOSTIC_SOURCE.to_owned()),
        message: format!(
            "Step definition is not used by any feature file: #[{}(\"{}\")]",
            step_type_to_attribute(step_def.keyword),
            step_def.pattern
        ),
        related_information: None,
        tags: None,
        data: None,
    }
}

/// Convert a `StepType` to the corresponding attribute name.
fn step_type_to_attribute(step_type: gherkin::StepType) -> &'static str {
    match step_type {
        gherkin::StepType::Given => "given",
        gherkin::StepType::When => "when",
        gherkin::StepType::Then => "then",
    }
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
    use lsp_types::{DidSaveTextDocumentParams, TextDocumentIdentifier};
    use rstest::{fixture, rstest};
    use std::path::PathBuf;
    use tempfile::TempDir;

    /// Test scenario components produced by the fixture.
    struct TestScenario {
        #[expect(dead_code, reason = "kept alive to preserve temp directory")]
        dir: TempDir,
        feature_path: PathBuf,
        rust_path: PathBuf,
        state: ServerState,
    }

    /// Builder for constructing test scenarios with specific file contents.
    struct ScenarioBuilder {
        dir: TempDir,
        state: ServerState,
    }

    impl ScenarioBuilder {
        fn index_file(&mut self, path: &std::path::Path) {
            let uri = Url::from_file_path(path).expect("file URI");
            let params = DidSaveTextDocumentParams {
                text_document: TextDocumentIdentifier { uri },
                text: None,
            };
            handle_did_save_text_document(&mut self.state, params);
        }

        fn with_files(mut self, feature_content: &str, rust_content: &str) -> TestScenario {
            let feature_path = self.dir.path().join("test.feature");
            let rust_path = self.dir.path().join("steps.rs");

            std::fs::write(&feature_path, feature_content).expect("write feature");
            std::fs::write(&rust_path, rust_content).expect("write rust");

            // Index feature file first, then Rust file
            self.index_file(&feature_path);
            self.index_file(&rust_path);

            TestScenario {
                dir: self.dir,
                feature_path,
                rust_path,
                state: self.state,
            }
        }
    }

    /// Fixture providing the infrastructure for diagnostic tests.
    #[fixture]
    fn scenario_builder() -> ScenarioBuilder {
        ScenarioBuilder {
            dir: TempDir::new().expect("temp dir"),
            state: ServerState::new(ServerConfig::default()),
        }
    }

    /// Helper to compute feature diagnostics for a path.
    fn compute_feature_diagnostics_for_path(
        state: &ServerState,
        feature_path: &Path,
    ) -> Vec<Diagnostic> {
        let feature_index = state.feature_index(feature_path).expect("feature index");
        compute_unimplemented_step_diagnostics(state, feature_index)
    }

    fn assert_feature_has_no_unimplemented_steps(state: &ServerState, feature_path: &Path) {
        let diags = compute_feature_diagnostics_for_path(state, feature_path);
        assert!(
            diags.is_empty(),
            "expected no unimplemented steps, found {}",
            diags.len()
        );
    }

    fn assert_rust_has_no_unused_steps(state: &ServerState, rust_path: &Path) {
        let diags = compute_unused_step_diagnostics(state, rust_path);
        assert!(
            diags.is_empty(),
            "expected no unused step definitions, found {}",
            diags.len()
        );
    }

    #[rstest]
    fn unimplemented_step_produces_diagnostic(scenario_builder: ScenarioBuilder) {
        let scenario = scenario_builder.with_files(
            "Feature: test\n  Scenario: s\n    Given an unimplemented step\n",
            concat!(
                "use rstest_bdd_macros::given;\n\n",
                "#[given(\"a different step\")]\n",
                "fn diff() {}\n",
            ),
        );

        let feature_index = scenario
            .state
            .feature_index(&scenario.feature_path)
            .expect("index");
        let diagnostics = compute_unimplemented_step_diagnostics(&scenario.state, feature_index);

        assert_eq!(diagnostics.len(), 1);
        let diag = diagnostics.first().expect("diagnostic");
        assert_eq!(diag.severity, Some(DiagnosticSeverity::WARNING));
        assert!(diag.message.contains("an unimplemented step"));
        assert_eq!(
            diag.code,
            Some(lsp_types::NumberOrString::String(
                CODE_UNIMPLEMENTED_STEP.to_owned()
            ))
        );
    }

    #[rstest]
    fn unused_step_definition_produces_diagnostic(scenario_builder: ScenarioBuilder) {
        let scenario = scenario_builder.with_files(
            "Feature: test\n  Scenario: s\n    Given a step\n",
            concat!(
                "use rstest_bdd_macros::given;\n\n",
                "#[given(\"a step\")]\n",
                "fn step() {}\n\n",
                "#[given(\"unused step\")]\n",
                "fn unused() {}\n",
            ),
        );

        let diagnostics = compute_unused_step_diagnostics(&scenario.state, &scenario.rust_path);

        assert_eq!(diagnostics.len(), 1);
        let diag = diagnostics.first().expect("diagnostic");
        assert!(diag.message.contains("unused step"));
        assert_eq!(
            diag.code,
            Some(lsp_types::NumberOrString::String(
                CODE_UNUSED_STEP_DEFINITION.to_owned()
            ))
        );
    }

    #[rstest]
    #[case::implemented_step_no_feature_diagnostic(
        "Feature: test\n  Scenario: s\n    Given a step\n",
        concat!(
            "use rstest_bdd_macros::given;\n\n",
            "#[given(\"a step\")]\n",
            "fn step() {}\n",
        ),
        "feature"
    )]
    #[case::used_step_definition_no_rust_diagnostic(
        "Feature: test\n  Scenario: s\n    Given a step\n",
        concat!(
            "use rstest_bdd_macros::given;\n\n",
            "#[given(\"a step\")]\n",
            "fn step() {}\n",
        ),
        "rust"
    )]
    #[case::parameterized_pattern_no_diagnostics(
        "Feature: test\n  Scenario: s\n    Given I have 5 items\n",
        concat!(
            "use rstest_bdd_macros::given;\n\n",
            "#[given(\"I have {n:u32} items\")]\n",
            "fn items() {}\n",
        ),
        "both"
    )]
    fn no_diagnostics_scenarios(
        scenario_builder: ScenarioBuilder,
        #[case] feature_content: &str,
        #[case] rust_content: &str,
        #[case] check_type: &str,
    ) {
        let scenario = scenario_builder.with_files(feature_content, rust_content);
        match check_type {
            "feature" => {
                assert_feature_has_no_unimplemented_steps(&scenario.state, &scenario.feature_path);
            }
            "rust" => assert_rust_has_no_unused_steps(&scenario.state, &scenario.rust_path),
            "both" => {
                assert_feature_has_no_unimplemented_steps(&scenario.state, &scenario.feature_path);
                assert_rust_has_no_unused_steps(&scenario.state, &scenario.rust_path);
            }
            _ => panic!("invalid check_type: {check_type}"),
        }
    }

    #[rstest]
    fn keyword_matching_is_enforced(scenario_builder: ScenarioBuilder) {
        // Given step should not match When implementation
        let scenario = scenario_builder.with_files(
            "Feature: test\n  Scenario: s\n    Given a step\n",
            concat!(
                "use rstest_bdd_macros::when;\n\n",
                "#[when(\"a step\")]\n",
                "fn step() {}\n",
            ),
        );

        // Feature step should be unimplemented (Given != When)
        let feature_diags =
            compute_feature_diagnostics_for_path(&scenario.state, &scenario.feature_path);
        assert_eq!(feature_diags.len(), 1, "keyword mismatch should be caught");

        // Rust step should be unused (When != Given)
        let rust_diags = compute_unused_step_diagnostics(&scenario.state, &scenario.rust_path);
        assert_eq!(rust_diags.len(), 1, "When step should be unused");
    }

    #[test]
    fn step_type_to_attribute_returns_correct_names() {
        assert_eq!(step_type_to_attribute(gherkin::StepType::Given), "given");
        assert_eq!(step_type_to_attribute(gherkin::StepType::When), "when");
        assert_eq!(step_type_to_attribute(gherkin::StepType::Then), "then");
    }
}
