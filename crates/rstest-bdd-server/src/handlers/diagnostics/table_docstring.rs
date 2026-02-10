//! Table and docstring expectation validation diagnostics.
//!
//! This module validates that feature steps have tables and docstrings that
//! match what the corresponding Rust implementation expects.

use std::sync::Arc;

use lsp_types::Diagnostic;
use rstest_bdd_patterns::SpecificityScore;

use crate::indexing::{CompiledStepDefinition, FeatureFileIndex, IndexedStep};
use crate::server::ServerState;

use super::compute::{DiagnosticSpec, FeatureStepDiagnosticKind, build_step_diagnostic};
use super::{
    CODE_DOCSTRING_EXPECTED, CODE_DOCSTRING_NOT_EXPECTED, CODE_TABLE_EXPECTED,
    CODE_TABLE_NOT_EXPECTED,
};
use crate::handlers::util::gherkin_span_to_lsp_range;

/// Compute diagnostics for table/docstring expectation mismatches.
///
/// For each feature step, checks if the step has a table or docstring and
/// whether the matching Rust implementation expects them.
///
/// # Example
///
/// ```no_run
/// use rstest_bdd_server::config::ServerConfig;
/// use rstest_bdd_server::server::ServerState;
/// use rstest_bdd_server::indexing::FeatureFileIndex;
///
/// let state = ServerState::new(ServerConfig::default());
/// // Obtain a FeatureFileIndex from state.feature_index(path)
/// # let feature_index: FeatureFileIndex = todo!();
///
/// let diagnostics = rstest_bdd_server::handlers::compute_table_docstring_mismatch_diagnostics(
///     &state,
///     &feature_index,
/// );
/// // Returns diagnostics when feature steps have tables/docstrings
/// // but the Rust implementation doesn't expect them (or vice versa)
/// for diag in &diagnostics {
///     println!("{}", diag.message);
/// }
/// ```
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

    if let Some(diag) = StepArgumentKind::Table.check_expectation(feature_index, step, &impl_def) {
        diagnostics.push(diag);
    }

    if let Some(diag) =
        StepArgumentKind::Docstring.check_expectation(feature_index, step, &impl_def)
    {
        diagnostics.push(diag);
    }

    diagnostics
}

/// The type of step argument expectation to validate.
enum StepArgumentKind {
    /// Data table expectation.
    Table,
    /// Docstring expectation.
    Docstring,
}

impl StepArgumentKind {
    /// Check if the step's argument matches the implementation's expectation.
    ///
    /// Returns a diagnostic if there's a mismatch:
    /// - Step has the argument but impl doesn't expect it
    /// - Impl expects the argument but step doesn't have it
    fn check_expectation(
        self,
        feature_index: &FeatureFileIndex,
        step: &IndexedStep,
        impl_def: &CompiledStepDefinition,
    ) -> Option<Diagnostic> {
        let (step_has_arg, impl_expects_arg, not_expected_kind, expected_kind) = match self {
            Self::Table => (
                step.table.is_some(),
                impl_def.expects_table,
                FeatureStepDiagnosticKind::TableNotExpected,
                FeatureStepDiagnosticKind::TableExpected,
            ),
            Self::Docstring => (
                step.docstring.is_some(),
                impl_def.expects_docstring,
                FeatureStepDiagnosticKind::DocstringNotExpected,
                FeatureStepDiagnosticKind::DocstringExpected,
            ),
        };

        if step_has_arg && !impl_expects_arg {
            Some(not_expected_kind.build(feature_index, step))
        } else if !step_has_arg && impl_expects_arg {
            Some(expected_kind.build(feature_index, step))
        } else {
            None
        }
    }
}

impl FeatureStepDiagnosticKind {
    /// Build a diagnostic from this kind for the given step.
    fn build(self, feature_index: &FeatureFileIndex, step: &IndexedStep) -> Diagnostic {
        let spec = match self {
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
            // UnimplementedStep is handled in compute.rs
            Self::UnimplementedStep { .. } => {
                unreachable!("UnimplementedStep should not be built via table_docstring module")
            }
        };

        build_step_diagnostic(feature_index, step, spec)
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
