//! Text document notification handlers.
//!
//! Phase 7 focuses on building language-server foundations. This module
//! provides the on-save indexing pipeline for `.feature` files and Rust step
//! definition sources. Indexing results are stored in the shared server state.
//! After indexing, diagnostics are computed and published via the LSP protocol.

use lsp_types::DidSaveTextDocumentParams;
use metrics::{counter, describe_counter};
use tracing::{debug, warn};

use super::{
    diagnostics::{,
    FeatureDiagnosticPublication,
    clear_rust_index_diagnostics,
    publish_all_feature_diagnostics,
    publish_feature_diagnostics,
    publish_rust_index_result_diagnostics,
    publish_rust_diagnostics,
    },
    util::has_extension,
    workspace_metrics::{record_deferred_save_depth, record_workspace_outcome},
};
use crate::{
    indexing::{,
    FeatureIndexError,
    RustStepIndexError,
    index_feature_source,
    index_rust_file,
    index_rust_source,
    server::ServerState,
    indexing::{index_feature_file, index_feature_source, index_rust_file, index_rust_source},
    },
    server::ServerState,
};


//! Text document notification handlers.
//!
//! Phase 7 focuses on building language-server foundations. This module
//! provides the on-save indexing pipeline for `.feature` files and Rust step
//! definition sources. Indexing results are stored in the shared server state.
//! After indexing, diagnostics are computed and published via the LSP protocol.
    diagnostics::{
};
    indexing::{
};

const INDEXING_COUNTER: &str = "rstest_bdd_server_indexing_total";

fn record_indexing_outcome(operation: &'static str, outcome: &'static str) {
    describe_counter!(
        INDEXING_COUNTER,
        "Language-server indexing outcomes, labelled by operation and outcome"
    );
    counter!(INDEXING_COUNTER, "operation" => operation, "outcome" => outcome).increment(1);
}

fn feature_indexing_outcome(error: &FeatureIndexError) -> &'static str {
    match error {
        FeatureIndexError::WorkspaceRootUnavailable => "workspace-root-unavailable",
        FeatureIndexError::OutsideWorkspaceRoot { .. } => "workspace-boundary-failure",
        FeatureIndexError::NonUtf8Path { .. } => "non-utf8-path",
        FeatureIndexError::Read(_) => "read-failure",
        FeatureIndexError::Parse(_) => "parse-failure",
        FeatureIndexError::DocstringSpanNotFound(_) => "docstring-span-failure",
    }
}

fn rust_indexing_outcome(error: &RustStepIndexError) -> &'static str {
    match error {
        RustStepIndexError::Read(_) => "read-failure",
        RustStepIndexError::Parse(_) => "parse-failure",
    }
}

/// Handle `textDocument/didSave` notifications.
///
/// When a saved document is a `.feature` file or a Rust source file, the
/// server parses and indexes it. After successful indexing, diagnostics are
/// computed and published. Parse failures are logged but do not produce
/// diagnostics (the file remains in its previously indexed state).
pub fn handle_did_save_text_document(state: &mut ServerState, params: DidSaveTextDocumentParams) {
    if state.workspace_preparation_pending() {
        match state.defer_document_save(params) {
            Ok(depth) => {
                record_workspace_outcome("deferred-save", "queued");
                record_deferred_save_depth(depth);
            }
            Err(reason) => {
                record_workspace_outcome("deferred-save", reason.metric_outcome());
                record_deferred_save_depth(state.deferred_document_save_count());
                warn!(
                    ?reason,
                    "discarding deferred didSave because its bounded queue is full"
                );
            }
        }
        return;
    }
    index_saved_document(state, params);
}

pub(super) fn index_saved_document(state: &mut ServerState, params: DidSaveTextDocumentParams) {
    let uri = params.text_document.uri;
    let Ok(path) = uri.to_file_path() else {
        debug!(%uri, "ignoring didSave for non-file URI");
        return;
    };

    if has_extension(&path, "feature") {
        handle_feature_file_save(state, &path, params.text.as_deref());
    } else if has_extension(&path, "rs") {
        handle_rust_file_save(state, &path, params.text.as_deref());
    }
}

fn index_saved_source<T, E>(
    path: &std::path::Path,
    text: Option<&str>,
    index_file: impl FnOnce(&std::path::Path) -> Result<T, E>,
    index_source: impl FnOnce(std::path::PathBuf, &str) -> Result<T, E>,
) -> Result<T, E> {
    text.map_or_else(
        || index_file(path),
        |source| index_source(path.to_path_buf(), source),
    )
}

fn handle_feature_file_save(state: &mut ServerState, path: &std::path::Path, text: Option<&str>) {
    let index_result = index_saved_source(
        path,
        text,
        |path| state.index_feature_file(path),
        index_feature_source,
    );

    apply_feature_index_result(
        state,
        path,
        index_result,
        FeatureDiagnosticPublication::Immediate,
    );
}

pub(super) fn apply_feature_index_result(
    state: &mut ServerState,
    path: &std::path::Path,
    index_result: Result<crate::indexing::FeatureFileIndex, FeatureIndexError>,
    diagnostic_publication: FeatureDiagnosticPublication,
) {
    match index_result {
        Ok(index) => {
            record_indexing_outcome("feature", "success");
            debug!(
                path = %path.display(),
                steps = index.steps.len(),
                examples = index.example_columns.len(),
                "indexed feature file"
            );
            state.upsert_feature_index(index);
            if matches!(
                diagnostic_publication,
                FeatureDiagnosticPublication::Immediate
            ) {
                publish_feature_diagnostics(state, path);
            }
        }
        Err(err) => {
            record_indexing_outcome("feature", feature_indexing_outcome(&err));
            warn!(path = %path.display(), error = %err, "failed to index feature file");
        }
    }
}

fn handle_rust_file_save(state: &mut ServerState, path: &std::path::Path, text: Option<&str>) {
    let index_result = index_saved_source(path, text, index_rust_file, index_rust_source);

    apply_rust_index_result(
        state,
        path,
        index_result,
        FeatureDiagnosticPublication::Immediate,
    );
}

pub(super) fn apply_rust_index_result(
    state: &mut ServerState,
    path: &std::path::Path,
    index_result: Result<crate::indexing::RustStepIndexResult, RustStepIndexError>,
    diagnostic_publication: FeatureDiagnosticPublication,
) {
    match index_result {
        Ok(result) => {
            record_indexing_outcome("rust", "success");
            for _ in &result.diagnostics {
                record_indexing_outcome("rust", "recoverable-diagnostic");
            }
            let index = result.index;
            debug!(
                path = %path.display(),
                steps = index.step_definitions.len(),
                "indexed rust step file"
            );
            state.upsert_rust_step_index(index);
            publish_rust_index_result_diagnostics(state, path, &result.diagnostics);
            for diagnostic in result.diagnostics {
                warn!(path = %path.display(), error = %diagnostic, "indexed Rust file with a step diagnostic");
            }
            if matches!(
                diagnostic_publication,
                FeatureDiagnosticPublication::Immediate
            ) {
                publish_all_feature_diagnostics(state);
            }
        }
        Err(err) => {
            record_indexing_outcome("rust", rust_indexing_outcome(&err));
            clear_rust_index_diagnostics(state, path);
            warn!(path = %path.display(), error = %err, "failed to index rust step file");
        }
    }
}

#[cfg(test)]
mod tests {
    //! Recorder-backed tests for language-server indexing metrics.

    use std::sync::{Arc, Mutex};

    use ::metrics::{
        Counter,
        CounterFn,
        Gauge,
        Histogram,
        Key,
        KeyName,
        Metadata,
        Recorder,
        SharedString,
        Unit,
        with_local_recorder,
    };
    use lsp_types::{TextDocumentIdentifier, Url};
    use tempfile::TempDir;

    use super::*;
    use crate::{config::ServerConfig, discovery::WorkspaceInfo, server::ServerState};

    #[derive(Default)]
    struct IndexingRecorder {
        outcomes: Arc<Mutex<Vec<(String, String, u64)>>>,
        counters: Arc<Mutex<Vec<RegisteredCounter>>>,
    }

    #[derive(Clone, Debug, PartialEq, Eq)]
    struct RegisteredCounter {
        name: String,
        labels: Vec<(String, String)>,
    }

    struct RecordedCounter {
        outcomes: Arc<Mutex<Vec<(String, String, u64)>>>,
        operation: String,
        outcome: String,
    }

    impl CounterFn for RecordedCounter {
        fn increment(&self, value: u64) {
            let mut outcomes = match self.outcomes.lock() {
                Ok(outcomes) => outcomes,
                Err(error) => error.into_inner(),
            };
            let Some((_, _, count)) = outcomes.iter_mut().find(|(operation, outcome, _)| {
                operation == &self.operation && outcome == &self.outcome
            }) else {
                outcomes.push((self.operation.clone(), self.outcome.clone(), value));
                return;
            };
            *count += value;
        }

        fn absolute(&self, value: u64) {
            let mut outcomes = match self.outcomes.lock() {
                Ok(outcomes) => outcomes,
                Err(error) => error.into_inner(),
            };
            let Some((_, _, count)) = outcomes.iter_mut().find(|(operation, outcome, _)| {
                operation == &self.operation && outcome == &self.outcome
            }) else {
                outcomes.push((self.operation.clone(), self.outcome.clone(), value));
                return;
            };
            *count = (*count).max(value);
        }
    }

    impl IndexingRecorder {
        fn count(&self, operation: &str, outcome: &str) -> u64 {
            let outcomes = match self.outcomes.lock() {
                Ok(outcomes) => outcomes,
                Err(error) => error.into_inner(),
            };
            outcomes
                .iter()
                .find_map(|(recorded_operation, recorded_outcome, count)| {
                    (recorded_operation == operation && recorded_outcome == outcome)
                        .then_some(*count)
                })
                .unwrap_or_default()
        }

        fn registered_counters(&self) -> Vec<RegisteredCounter> {
            let counters = match self.counters.lock() {
                Ok(counters) => counters,
                Err(error) => error.into_inner(),
            };
            counters.clone()
        }
    }

    impl Recorder for IndexingRecorder {
        fn describe_counter(&self, _: KeyName, _: Option<Unit>, _: SharedString) {}

        fn describe_gauge(&self, _: KeyName, _: Option<Unit>, _: SharedString) {}

        fn describe_histogram(&self, _: KeyName, _: Option<Unit>, _: SharedString) {}

        fn register_counter(&self, key: &Key, _: &Metadata<'_>) -> Counter {
            if key.name() != INDEXING_COUNTER {
                return Counter::noop();
            }
            let operation = key
                .labels()
                .find(|label| label.key() == "operation")
                .map(|label| label.value().to_owned());
            let outcome = key
                .labels()
                .find(|label| label.key() == "outcome")
                .map(|label| label.value().to_owned());
            let mut labels: Vec<_> = key
                .labels()
                .map(|label| (label.key().to_owned(), label.value().to_owned()))
                .collect();
            labels.sort_unstable();
            let mut counters = match self.counters.lock() {
                Ok(counters) => counters,
                Err(error) => error.into_inner(),
            };
            counters.push(RegisteredCounter {
                name: key.name().to_owned(),
                labels,
            });
            drop(counters);
            match (operation, outcome) {
                (Some(operation), Some(outcome)) => Counter::from_arc(Arc::new(RecordedCounter {
                    outcomes: Arc::clone(&self.outcomes),
                    operation,
                    outcome,
                })),
                _ => Counter::noop(),
            }
        }

        fn register_gauge(&self, _: &Key, _: &Metadata<'_>) -> Gauge { Gauge::noop() }

        fn register_histogram(&self, _: &Key, _: &Metadata<'_>) -> Histogram { Histogram::noop() }
    }

    fn did_save_params(path: &std::path::Path, text: Option<&str>) -> DidSaveTextDocumentParams {
        let Ok(uri) = Url::from_file_path(path) else {
            panic!("test path must convert to URI: {}", path.display());
        };
        DidSaveTextDocumentParams {
            text_document: TextDocumentIdentifier { uri },
            text: text.map(str::to_owned),
        }
    }

    mod metrics;
}
