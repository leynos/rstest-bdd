//! Diagnostic computation logic.
//!
//! This module contains the core algorithms for computing diagnostics:
//! - Checking for unimplemented feature steps
//! - Checking for unused step definitions
//! - Checking for placeholder count mismatches
//! - Checking for table/docstring expectation mismatches

use std::collections::HashSet;
use std::{path::Path, sync::Arc};

use lsp_types::{Diagnostic, DiagnosticSeverity, Range};
use rstest_bdd_patterns::SpecificityScore;
use rstest_bdd_patterns::pattern::lexer::{Token, lex_pattern};

use crate::indexing::{
    CompiledStepDefinition, FeatureFileIndex, IndexedStep, IndexedStepParameter,
};
use crate::server::ServerState;

use super::{
    CODE_DOCSTRING_EXPECTED, CODE_DOCSTRING_NOT_EXPECTED, CODE_PLACEHOLDER_COUNT_MISMATCH,
    CODE_TABLE_EXPECTED, CODE_TABLE_NOT_EXPECTED, CODE_UNIMPLEMENTED_STEP,
    CODE_UNUSED_STEP_DEFINITION, DIAGNOSTIC_SOURCE,
};
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

/// Specification for building a step diagnostic.
struct DiagnosticSpec {
    code: &'static str,
    message: String,
    custom_range: Option<Range>,
}

/// Build a diagnostic for a feature step from a specification.
///
/// Uses `spec.custom_range` if provided, otherwise computes the range from `step.span`.
fn build_step_diagnostic(
    feature_index: &FeatureFileIndex,
    step: &IndexedStep,
    spec: DiagnosticSpec,
) -> Diagnostic {
    let range = spec
        .custom_range
        .unwrap_or_else(|| gherkin_span_to_lsp_range(&feature_index.source, step.span));

    Diagnostic {
        range,
        severity: Some(DiagnosticSeverity::WARNING),
        code: Some(lsp_types::NumberOrString::String(spec.code.to_owned())),
        code_description: None,
        source: Some(DIAGNOSTIC_SOURCE.to_owned()),
        message: spec.message,
        related_information: None,
        tags: None,
        data: None,
    }
}

/// Kinds of diagnostics that can be reported for feature steps.
enum FeatureStepDiagnosticKind {
    /// No Rust implementation found for the step.
    UnimplementedStep { keyword: String, text: String },
    /// Feature step has a data table but implementation doesn't expect one.
    TableNotExpected,
    /// Implementation expects a data table but feature step doesn't have one.
    TableExpected,
    /// Feature step has a docstring but implementation doesn't expect one.
    DocstringNotExpected,
    /// Implementation expects a docstring but feature step doesn't have one.
    DocstringExpected,
}

impl FeatureStepDiagnosticKind {
    /// Build a diagnostic from this kind for the given step.
    fn build(self, feature_index: &FeatureFileIndex, step: &IndexedStep) -> Diagnostic {
        let spec = match self {
            Self::UnimplementedStep { keyword, text } => DiagnosticSpec {
                code: CODE_UNIMPLEMENTED_STEP,
                message: format!("No Rust implementation found for {keyword} step: \"{text}\""),
                custom_range: None,
            },
            Self::TableNotExpected => DiagnosticSpec {
                code: CODE_TABLE_NOT_EXPECTED,
                message: "Data table provided but step implementation does not expect one"
                    .to_owned(),
                custom_range: step
                    .table
                    .as_ref()
                    .map(|t| gherkin_span_to_lsp_range(&feature_index.source, t.span)),
            },
            Self::TableExpected => DiagnosticSpec {
                code: CODE_TABLE_EXPECTED,
                message: "Step implementation expects a data table but none is provided".to_owned(),
                custom_range: None,
            },
            Self::DocstringNotExpected => DiagnosticSpec {
                code: CODE_DOCSTRING_NOT_EXPECTED,
                message: "Doc string provided but step implementation does not expect one"
                    .to_owned(),
                custom_range: step
                    .docstring
                    .as_ref()
                    .map(|d| gherkin_span_to_lsp_range(&feature_index.source, d.span)),
            },
            Self::DocstringExpected => DiagnosticSpec {
                code: CODE_DOCSTRING_EXPECTED,
                message: "Step implementation expects a doc string but none is provided".to_owned(),
                custom_range: None,
            },
        };

        build_step_diagnostic(feature_index, step, spec)
    }
}

/// Build a diagnostic for an unimplemented feature step.
fn build_unimplemented_step_diagnostic(
    feature_index: &FeatureFileIndex,
    step: &IndexedStep,
) -> Diagnostic {
    FeatureStepDiagnosticKind::UnimplementedStep {
        keyword: step.keyword.clone(),
        text: step.text.clone(),
    }
    .build(feature_index, step)
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
    let range = step_def.attribute_span.to_lsp_range();

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

// ============================================================================
// Placeholder count validation
// ============================================================================

/// Compute diagnostics for signature mismatches in step definitions.
///
/// Checks that each step definition's placeholder count matches the number of
/// step arguments in the function signature. A step argument is a function
/// parameter whose normalised name appears in the pattern's placeholder set.
#[must_use]
pub fn compute_signature_mismatch_diagnostics(
    state: &ServerState,
    rust_path: &Path,
) -> Vec<Diagnostic> {
    state
        .step_registry()
        .steps_for_file(rust_path)
        .iter()
        .filter_map(check_placeholder_count_mismatch)
        .collect()
}

/// Check if a step definition has a placeholder count mismatch.
///
/// Returns `Some(Diagnostic)` if the number of placeholders in the pattern
/// differs from the number of step arguments in the function signature.
fn check_placeholder_count_mismatch(step_def: &Arc<CompiledStepDefinition>) -> Option<Diagnostic> {
    let placeholder_names = extract_placeholder_names(&step_def.pattern)?;
    let placeholder_count = placeholder_names.len();
    let step_arg_count = count_step_arguments(&step_def.parameters, &placeholder_names);

    if placeholder_count == step_arg_count {
        return None;
    }

    Some(build_placeholder_mismatch_diagnostic(
        step_def,
        placeholder_count,
        step_arg_count,
    ))
}

/// Extract placeholder names from a step pattern.
///
/// Uses `lex_pattern()` as the single source of truth for placeholder parsing.
/// Returns `None` if the pattern cannot be lexed (malformed patterns are
/// handled elsewhere and should not produce additional diagnostics here).
fn extract_placeholder_names(pattern: &str) -> Option<HashSet<String>> {
    let tokens = lex_pattern(pattern).ok()?;
    let names = tokens
        .into_iter()
        .filter_map(|token| match token {
            Token::Placeholder { name, .. } => Some(normalise_param_name(&name)),
            _ => None,
        })
        .collect();
    Some(names)
}

/// Count step arguments among the function parameters.
///
/// A step argument is a parameter that:
/// 1. Is not a datatable parameter
/// 2. Is not a docstring parameter
/// 3. Has a normalised name that appears in the placeholder set
fn count_step_arguments(
    parameters: &[IndexedStepParameter],
    placeholder_names: &HashSet<String>,
) -> usize {
    parameters
        .iter()
        .filter(|param| !param.is_datatable && !param.is_docstring)
        .filter(|param| {
            param
                .name
                .as_ref()
                .is_some_and(|name| placeholder_names.contains(&normalise_param_name(name)))
        })
        .count()
}

/// Normalise a parameter or placeholder name for comparison.
///
/// Strips a single leading underscore to match the macro behaviour, where
/// users prefix parameters with `_` to suppress unused warnings.
fn normalise_param_name(name: &str) -> String {
    name.strip_prefix('_').unwrap_or(name).to_owned()
}

/// Build a diagnostic for a placeholder count mismatch.
fn build_placeholder_mismatch_diagnostic(
    step_def: &Arc<CompiledStepDefinition>,
    placeholder_count: usize,
    step_arg_count: usize,
) -> Diagnostic {
    let range = Range {
        start: lsp_types::Position::new(step_def.line, 0),
        end: lsp_types::Position::new(step_def.line + 1, 0),
    };

    Diagnostic {
        range,
        severity: Some(DiagnosticSeverity::WARNING),
        code: Some(lsp_types::NumberOrString::String(
            CODE_PLACEHOLDER_COUNT_MISMATCH.to_owned(),
        )),
        code_description: None,
        source: Some(DIAGNOSTIC_SOURCE.to_owned()),
        message: format!(
            "Placeholder count mismatch: pattern has {} placeholder(s) but function has {} \
             step argument(s) - #[{}(\"{}\")]",
            placeholder_count,
            step_arg_count,
            step_type_to_attribute(step_def.keyword),
            step_def.pattern
        ),
        related_information: None,
        tags: None,
        data: None,
    }
}

// ============================================================================
// Table and docstring expectation validation
// ============================================================================

/// Compute diagnostics for table/docstring expectation mismatches.
///
/// For each feature step, checks if the step has a table or docstring and
/// whether the matching Rust implementation expects them.
#[must_use]
pub fn compute_table_docstring_mismatch_diagnostics(
    state: &ServerState,
    feature_index: &FeatureFileIndex,
) -> Vec<Diagnostic> {
    feature_index
        .steps
        .iter()
        .flat_map(|step| check_table_docstring_mismatches(state, feature_index, step))
        .collect()
}

/// Check a single feature step for table/docstring mismatches.
fn check_table_docstring_mismatches(
    state: &ServerState,
    feature_index: &FeatureFileIndex,
    step: &IndexedStep,
) -> Vec<Diagnostic> {
    let mut diagnostics = Vec::new();

    // Find the best matching implementation
    let matching_impl = find_best_matching_implementation(state, step);
    let Some(impl_def) = matching_impl else {
        // No implementation found - handled by unimplemented step diagnostic
        return diagnostics;
    };

    if let Some(diag) = check_table_expectation(feature_index, step, &impl_def) {
        diagnostics.push(diag);
    }

    if let Some(diag) = check_docstring_expectation(feature_index, step, &impl_def) {
        diagnostics.push(diag);
    }

    diagnostics
}

/// Check if the step's table matches the implementation's expectation.
///
/// Returns a diagnostic if there's a mismatch:
/// - Step has a table but impl doesn't expect one
/// - Impl expects a table but step doesn't have one
fn check_table_expectation(
    feature_index: &FeatureFileIndex,
    step: &IndexedStep,
    impl_def: &CompiledStepDefinition,
) -> Option<Diagnostic> {
    let step_has_table = step.table.is_some();

    if step_has_table && !impl_def.expects_table {
        Some(FeatureStepDiagnosticKind::TableNotExpected.build(feature_index, step))
    } else if !step_has_table && impl_def.expects_table {
        Some(FeatureStepDiagnosticKind::TableExpected.build(feature_index, step))
    } else {
        None
    }
}

/// Check if the step's docstring matches the implementation's expectation.
///
/// Returns a diagnostic if there's a mismatch:
/// - Step has a docstring but impl doesn't expect one
/// - Impl expects a docstring but step doesn't have one
fn check_docstring_expectation(
    feature_index: &FeatureFileIndex,
    step: &IndexedStep,
    impl_def: &CompiledStepDefinition,
) -> Option<Diagnostic> {
    let step_has_docstring = step.docstring.is_some();

    if step_has_docstring && !impl_def.expects_docstring {
        Some(FeatureStepDiagnosticKind::DocstringNotExpected.build(feature_index, step))
    } else if !step_has_docstring && impl_def.expects_docstring {
        Some(FeatureStepDiagnosticKind::DocstringExpected.build(feature_index, step))
    } else {
        None
    }
}

/// Find the best matching Rust implementation for a feature step.
///
/// Returns the implementation with the highest specificity score if multiple
/// match. Uses the same scoring algorithm as the runtime registry to ensure
/// diagnostics are consistent with actual execution.
fn find_best_matching_implementation(
    state: &ServerState,
    step: &IndexedStep,
) -> Option<Arc<CompiledStepDefinition>> {
    state
        .step_registry()
        .steps_for_keyword(step.step_type)
        .iter()
        .filter(|compiled| compiled.regex.is_match(&step.text))
        .max_by(|a, b| {
            // Use SpecificityScore for consistent ordering with runtime resolution.
            // Patterns that fail to parse (shouldn't happen for compiled defs) sort last.
            let score_a = SpecificityScore::calculate(&a.pattern).ok();
            let score_b = SpecificityScore::calculate(&b.pattern).ok();
            score_a.cmp(&score_b)
        })
        .cloned()
}
