//! Diagnostic computation logic.
//!
//! This module contains the core algorithms for computing diagnostics:
//! - Checking for unimplemented feature steps
//! - Checking for unused step definitions

use std::{path::Path, sync::Arc};

use lsp_types::{Diagnostic, DiagnosticSeverity, Range};

use crate::indexing::{CompiledStepDefinition, FeatureFileIndex, IndexedStep};
use crate::server::ServerState;

use super::{CODE_UNIMPLEMENTED_STEP, CODE_UNUSED_STEP_DEFINITION, DIAGNOSTIC_SOURCE};
use crate::handlers::util::gherkin_span_to_lsp_range;

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
    // TODO: Use precise column/start/end span fields from CompiledStepDefinition
    // once available (see PR #383 review). Currently spans entire line as fallback.
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
pub(super) fn step_type_to_attribute(step_type: gherkin::StepType) -> &'static str {
    match step_type {
        gherkin::StepType::Given => "given",
        gherkin::StepType::When => "when",
        gherkin::StepType::Then => "then",
    }
}
