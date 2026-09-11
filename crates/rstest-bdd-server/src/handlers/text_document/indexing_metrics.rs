//! Indexing-outcome metrics shared by the save handlers.
use metrics::{counter, describe_counter};

use crate::indexing::{FeatureIndexError, RustStepIndexError};

/// Metric name for indexing outcomes.
pub(super) const INDEXING_COUNTER: &str = "rstest_bdd_server_indexing_total";

/// Record one indexing operation outcome.
pub(super) fn record_indexing_outcome(operation: &'static str, outcome: &'static str) {
    describe_counter!(
        INDEXING_COUNTER,
        "Language-server indexing outcomes, labelled by operation and outcome"
    );
    counter!(INDEXING_COUNTER, "operation" => operation, "outcome" => outcome).increment(1);
}

/// Convert a feature indexing error to its fixed metric outcome.
pub(super) fn feature_indexing_outcome(error: &FeatureIndexError) -> &'static str {
    match error {
        FeatureIndexError::WorkspaceRootUnavailable => "workspace-root-unavailable",
        FeatureIndexError::OutsideWorkspaceRoot { .. } => "workspace-boundary-failure",
        FeatureIndexError::NonUtf8Path { .. } => "non-utf8-path",
        FeatureIndexError::Read(_) => "read-failure",
        FeatureIndexError::Parse(_) => "parse-failure",
        FeatureIndexError::DocstringSpanNotFound(_) => "docstring-span-failure",
    }
}

/// Convert a Rust indexing error to its fixed metric outcome.
pub(super) fn rust_indexing_outcome(error: &RustStepIndexError) -> &'static str {
    match error {
        RustStepIndexError::Read(_) => "read-failure",
        RustStepIndexError::Parse(_) => "parse-failure",
    }
}
