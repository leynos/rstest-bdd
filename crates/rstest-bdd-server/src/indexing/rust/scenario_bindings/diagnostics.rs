//! Structured diagnostics for recoverable scenario-binding indexing failures.

use std::path::{Path, PathBuf};

use syn::spanned::Spanned;

/// Why one scenario binding could not contribute a library scope.
pub(super) enum BindingIndexFailure {
    /// The macro arguments did not match the supported binding grammar.
    Malformed(String),
    /// The binding did not include a feature file or directory.
    MissingPath,
}

/// Recoverable diagnostic emitted when one binding cannot select a scope.
pub(crate) struct ScenarioBindingIndexDiagnostic {
    /// Rust source file containing the ignored binding.
    pub(super) source_path: PathBuf,
    /// Source line of the binding arguments.
    source_line: usize,
    /// Source column of the binding arguments.
    source_column: usize,
    /// Reason the binding could not be indexed.
    failure: BindingIndexFailure,
}

impl ScenarioBindingIndexDiagnostic {
    /// Create a diagnostic from one ignored binding.
    pub(super) fn new(
        source_path: &Path,
        tokens: &proc_macro2::TokenStream,
        failure: BindingIndexFailure,
    ) -> Self {
        let start = tokens.span().start();
        Self {
            source_path: source_path.to_path_buf(),
            source_line: start.line,
            source_column: start.column,
            failure,
        }
    }

    /// Emit the deferred warning at the language-server application boundary.
    pub(crate) fn emit_warning(&self) {
        let (failure_category, error) = match &self.failure {
            BindingIndexFailure::Malformed(error) => ("malformed-arguments", error.as_str()),
            BindingIndexFailure::MissingPath => ("missing-path", "binding has no feature target"),
        };
        tracing::warn!(
            operation = "index-scenario-binding",
            source_path = %self.source_path.display(),
            source_line = self.source_line,
            source_column = self.source_column,
            failure_category,
            fallback_state = "global-library",
            error,
            "ignored scenario binding while indexing its closed step-library scope"
        );
    }
}
