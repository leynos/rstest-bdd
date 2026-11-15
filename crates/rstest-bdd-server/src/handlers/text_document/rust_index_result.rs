//! Applying a Rust indexing result to server state and publishing diagnostics.
use tracing::{debug, warn};

use super::indexing_metrics::{record_indexing_outcome, rust_indexing_outcome};
use crate::{
    handlers::diagnostics::{
        FeatureDiagnosticPublication,
        clear_rust_index_diagnostics,
        publish_all_feature_diagnostics,
        publish_rust_index_result_diagnostics,
    },
    indexing::{RustStepIndexDiagnostic, RustStepIndexError, RustStepIndexResult},
    server::ServerState,
};

/// Apply a Rust indexing result and publish its diagnostics.
pub(in crate::handlers) fn apply_rust_index_result(
    state: &mut ServerState,
    path: &std::path::Path,
    index_result: Result<RustStepIndexResult, RustStepIndexError>,
    diagnostic_publication: FeatureDiagnosticPublication,
) {
    match index_result {
        Ok(result) => apply_rust_index_success(state, path, result, diagnostic_publication),
        Err(err) => apply_rust_index_failure(state, path, &err),
    }
}

/// Record, store and publish a successful Rust indexing result.
fn apply_rust_index_success(
    state: &mut ServerState,
    path: &std::path::Path,
    result: RustStepIndexResult,
    diagnostic_publication: FeatureDiagnosticPublication,
) {
    record_rust_success_metrics(result.diagnostics.len());
    let index = result.index;
    debug!(
        path = %path.display(),
        steps = index.step_definitions.len(),
        "indexed rust step file"
    );
    state.upsert_rust_step_index(index);
    publish_rust_index_result_diagnostics(state, path, &result.diagnostics);
    warn_rust_step_diagnostics(path, &result.diagnostics);
    if matches!(
        diagnostic_publication,
        FeatureDiagnosticPublication::Immediate
    ) {
        publish_all_feature_diagnostics(state);
    }
}

/// Count one successful Rust index plus one outcome per recoverable diagnostic.
fn record_rust_success_metrics(diagnostic_count: usize) {
    record_indexing_outcome("rust", "success");
    for _ in 0..diagnostic_count {
        record_indexing_outcome("rust", "recoverable-diagnostic");
    }
}

/// Log each recoverable diagnostic raised while indexing a Rust file.
fn warn_rust_step_diagnostics(path: &std::path::Path, diagnostics: &[RustStepIndexDiagnostic]) {
    for diagnostic in diagnostics {
        warn!(path = %path.display(), error = %diagnostic, "indexed Rust file with a step diagnostic");
    }
}

/// Record a failed Rust indexing attempt and drop the file's stale index.
fn apply_rust_index_failure(
    state: &mut ServerState,
    path: &std::path::Path,
    err: &RustStepIndexError,
) {
    record_indexing_outcome("rust", rust_indexing_outcome(err));
    state.remove_rust_step_index(path);
    clear_rust_index_diagnostics(state, path);
    warn!(path = %path.display(), error = %err, "failed to index rust step file");
}
