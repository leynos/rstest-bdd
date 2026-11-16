//! Apply Rust source indexing results to the shared server state.
//!
//! A Rust save updates scenario bindings, the step index and the compiled
//! steps as one transition, so applying those results lives apart from the
//! notification plumbing that decides when a save runs and from the feature
//! pipeline that publishes feature diagnostics.

use std::path::Path;

use tracing::{debug, warn};

use super::{
    super::diagnostics::{
        FeatureDiagnosticPublication,
        clear_rust_index_diagnostics,
        publish_all_feature_diagnostics,
        publish_rust_index_result_diagnostics,
    },
    indexing_metrics::{record_indexing_outcome, rust_indexing_outcome},
};
use crate::{
    indexing::{
        RustSourceIndexResult,
        RustStepIndexDiagnostic,
        RustStepIndexError,
        RustStepIndexResult,
    },
    server::ServerState,
};

/// Apply internal scenario bindings before the existing Rust step-index result.
pub(in crate::handlers) fn apply_rust_source_index_result(
    state: &mut ServerState,
    path: &Path,
    index_result: Result<RustSourceIndexResult, RustStepIndexError>,
    diagnostic_publication: FeatureDiagnosticPublication,
) {
    let step_result = match index_result {
        Ok(result) => {
            let RustSourceIndexResult {
                steps,
                scenario_bindings,
                scenario_binding_diagnostics,
            } = result;
            state.upsert_rust_scenario_bindings(path, scenario_bindings);
            // Bindings decide which closed scope a feature selects, so a Rust
            // save can both create and resolve a conflict. Publishing here
            // keeps the warning off the read paths.
            state.report_indexed_scope_conflicts();
            for diagnostic in scenario_binding_diagnostics {
                record_indexing_outcome("scenario-binding", diagnostic.failure_category());
                diagnostic.emit_warning();
            }
            Ok(steps)
        }
        Err(error) => {
            state.upsert_rust_scenario_bindings(path, Vec::new());
            state.report_indexed_scope_conflicts();
            Err(error)
        }
    };
    apply_rust_index_result(state, path, step_result, diagnostic_publication);
}

/// Apply a Rust indexing result and publish its diagnostics.
fn apply_rust_index_result(
    state: &mut ServerState,
    path: &Path,
    index_result: Result<RustStepIndexResult, RustStepIndexError>,
    diagnostic_publication: FeatureDiagnosticPublication,
) {
    match index_result {
        Ok(result) => apply_rust_index_success(state, path, result, diagnostic_publication),
        Err(err) => apply_rust_index_failure(state, path, &err),
    }
}

/// Record a successful Rust index and publish the diagnostics it carries.
fn apply_rust_index_success(
    state: &mut ServerState,
    path: &Path,
    result: RustStepIndexResult,
    diagnostic_publication: FeatureDiagnosticPublication,
) {
    record_rust_index_outcomes(&result);
    let index = result.index;
    debug!(
        path = %path.display(),
        steps = index.step_definitions.len(),
        "indexed rust step file"
    );
    state.upsert_rust_step_index(index);
    publish_rust_index_result_diagnostics(state, path, &result.diagnostics);
    log_step_diagnostics(path, &result.diagnostics);
    publish_features_if_immediate(state, diagnostic_publication);
}

/// Record the success outcome, plus one per recoverable step diagnostic.
fn record_rust_index_outcomes(result: &RustStepIndexResult) {
    record_indexing_outcome("rust", "success");
    for _ in &result.diagnostics {
        record_indexing_outcome("rust", "recoverable-diagnostic");
    }
}

/// Warn about each step diagnostic carried by an otherwise successful index.
fn log_step_diagnostics(path: &Path, diagnostics: &[RustStepIndexDiagnostic]) {
    for diagnostic in diagnostics {
        warn!(path = %path.display(), error = %diagnostic, "indexed Rust file with a step diagnostic");
    }
}

/// Publish feature diagnostics when the save asked for immediate publication.
fn publish_features_if_immediate(
    state: &mut ServerState,
    diagnostic_publication: FeatureDiagnosticPublication,
) {
    if matches!(
        diagnostic_publication,
        FeatureDiagnosticPublication::Immediate
    ) {
        publish_all_feature_diagnostics(state);
    }
}

/// Record a failed Rust index and drop whatever the previous index published.
fn apply_rust_index_failure(state: &mut ServerState, path: &Path, err: &RustStepIndexError) {
    record_indexing_outcome("rust", rust_indexing_outcome(err));
    state.remove_rust_step_index(path);
    clear_rust_index_diagnostics(state, path);
    warn!(path = %path.display(), error = %err, "failed to index rust step file");
}
