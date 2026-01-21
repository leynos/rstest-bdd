//! Placeholder count validation diagnostics.
//!
//! This module validates that step definitions have matching placeholder and
//! step argument counts. A placeholder count mismatch indicates that the
//! function signature doesn't match the pattern's expectations.

use std::collections::HashSet;
use std::path::Path;
use std::sync::Arc;

use lsp_types::{Diagnostic, DiagnosticSeverity};
use rstest_bdd_patterns::pattern::lexer::{Token, lex_pattern};

use crate::indexing::{CompiledStepDefinition, IndexedStepParameter};
use crate::server::ServerState;

use super::compute::step_type_to_attribute;
use super::{CODE_PLACEHOLDER_COUNT_MISMATCH, DIAGNOSTIC_SOURCE};

/// Compute diagnostics for signature mismatches in step definitions.
///
/// Checks that each step definition's placeholder count matches the number of
/// step arguments in the function signature. A step argument is a function
/// parameter whose normalized name appears in the pattern's placeholder set.
///
/// # Example
///
/// ```no_run
/// use rstest_bdd_server::server::ServerState;
/// use std::path::Path;
///
/// let state = ServerState::default();
/// let rust_path = Path::new("steps.rs");
///
/// let diagnostics = rstest_bdd_server::handlers::compute_signature_mismatch_diagnostics(
///     &state,
///     rust_path,
/// );
/// // Returns diagnostics for any step where placeholder count != step argument count
/// for diag in &diagnostics {
///     println!("{}", diag.message);
/// }
/// ```
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
/// Returns `Some(Diagnostic)` if the number of placeholder occurrences in the
/// pattern differs from the number of step arguments in the function signature.
/// This mirrors the macro's `capture_count` semantics which counts every
/// placeholder occurrence, not just distinct names.
fn check_placeholder_count_mismatch(step_def: &Arc<CompiledStepDefinition>) -> Option<Diagnostic> {
    // Lex once and reuse tokens for both placeholder count and name extraction.
    let tokens = lex_pattern(&step_def.pattern).ok()?;

    let placeholder_count = count_placeholder_occurrences(&tokens);
    let placeholder_names = extract_placeholder_names(&tokens);
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

/// Count placeholder occurrences from pre-lexed tokens.
///
/// Returns the total number of `Token::Placeholder` occurrences to match the
/// macro's `capture_count` semantics (which counts every placeholder, not just
/// distinct names).
fn count_placeholder_occurrences(tokens: &[Token]) -> usize {
    tokens
        .iter()
        .filter(|token| matches!(token, Token::Placeholder { .. }))
        .count()
}

/// Extract placeholder names from pre-lexed tokens.
///
/// Returns a `HashSet` of normalized placeholder names for matching against
/// function parameter names.
fn extract_placeholder_names(tokens: &[Token]) -> HashSet<String> {
    tokens
        .iter()
        .filter_map(|token| match token {
            Token::Placeholder { name, .. } => Some(normalize_param_name(name)),
            _ => None,
        })
        .collect()
}

/// Count step arguments among the function parameters.
///
/// A step argument is a parameter that either:
/// - Is a step struct (`#[step_args]`) which bundles all placeholders, OR
/// - Is not a datatable/docstring parameter AND has a normalized name that
///   appears in the placeholder set
///
/// Step structs bypass the datatable/docstring exclusion because they are
/// explicitly marked as step arguments regardless of other attributes.
///
/// This distinguishes step arguments from fixture parameters, which do not
/// correspond to placeholders in the pattern.
fn count_step_arguments(
    parameters: &[IndexedStepParameter],
    placeholder_names: &HashSet<String>,
) -> usize {
    parameters
        .iter()
        .filter(|param| {
            // Step structs bundle all placeholders, so they count as step arguments
            // unconditionally—bypassing the datatable/docstring exclusion.
            if param.is_step_struct {
                return true;
            }

            // Regular parameters: exclude datatable/docstring, then check if
            // the normalized name matches a placeholder.
            if param.is_datatable || param.is_docstring {
                return false;
            }

            param
                .name
                .as_ref()
                .is_some_and(|name| placeholder_names.contains(&normalize_param_name(name)))
        })
        .count()
}

/// Normalize a parameter or placeholder name for comparison.
///
/// Strips a single leading underscore to match the macro behaviour, where
/// users prefix parameters with `_` to suppress unused warnings.
fn normalize_param_name(name: &str) -> String {
    name.strip_prefix('_').unwrap_or(name).to_owned()
}

/// Build a diagnostic for a placeholder count mismatch.
fn build_placeholder_mismatch_diagnostic(
    step_def: &Arc<CompiledStepDefinition>,
    placeholder_count: usize,
    step_arg_count: usize,
) -> Diagnostic {
    let range = step_def.attribute_span.to_lsp_range();

    Diagnostic {
        range,
        severity: Some(DiagnosticSeverity::WARNING),
        code: Some(lsp_types::NumberOrString::String(
            CODE_PLACEHOLDER_COUNT_MISMATCH.to_owned(),
        )),
        code_description: None,
        source: Some(DIAGNOSTIC_SOURCE.to_owned()),
        message: format!(
            "Placeholder count mismatch: pattern has {} placeholder(s) but \
             function has {} step argument(s) - #[{}(\"{}\")]",
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
