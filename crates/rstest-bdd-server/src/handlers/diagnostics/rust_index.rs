//! LSP publication for recoverable Rust step-indexing diagnostics.
//!
//! This module converts the transient diagnostics returned by Rust indexing
//! into LSP warnings. It owns the save-pipeline-only publishing seam so that
//! [`crate::server::ServerState`] continues to retain only reusable file
//! indexes, not per-save diagnostics.

use std::path::Path;

use lsp_types::{Diagnostic, DiagnosticSeverity, Range};

use crate::indexing::RustStepIndexDiagnostic;
use crate::server::ServerState;

use super::publish::{compute_rust_file_diagnostics, publish_with};
use super::{
    CODE_INVALID_STEP_ATTRIBUTE_ARGUMENTS, CODE_MULTIPLE_STEP_ATTRIBUTES, DIAGNOSTIC_SOURCE,
};

/// Publish all Rust-file diagnostics, including recoverable indexing failures.
///
/// The values belong to one indexing result, so a later valid save publishes
/// without them and clears the stale warning from the LSP client.
pub(crate) fn publish_rust_index_result_diagnostics(
    state: &ServerState,
    rust_path: &Path,
    indexing_diagnostics: &[RustStepIndexDiagnostic],
) {
    publish_with(
        state,
        rust_path,
        "failed to publish rust diagnostics",
        |state, rust_path| {
            Some(compute_rust_file_diagnostics(
                state,
                rust_path,
                indexing_diagnostics,
            ))
        },
    );
}

/// Convert a recoverable Rust indexing failure into an LSP warning.
///
/// Indexing diagnostics identify the affected function but do not retain a
/// source span, so the LSP range is the document origin.
pub(super) fn build_rust_index_diagnostic(diagnostic: &RustStepIndexDiagnostic) -> Diagnostic {
    let code = match diagnostic {
        RustStepIndexDiagnostic::MultipleStepAttributes { .. } => CODE_MULTIPLE_STEP_ATTRIBUTES,
        RustStepIndexDiagnostic::InvalidStepAttributeArguments { .. } => {
            CODE_INVALID_STEP_ATTRIBUTE_ARGUMENTS
        }
    };

    Diagnostic {
        range: Range::default(),
        severity: Some(DiagnosticSeverity::WARNING),
        code: Some(lsp_types::NumberOrString::String(code.to_owned())),
        code_description: None,
        source: Some(DIAGNOSTIC_SOURCE.to_owned()),
        message: diagnostic.to_string(),
        related_information: None,
        tags: None,
        data: None,
    }
}
