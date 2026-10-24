//! Scope-aware compile-time step validation and diagnostics.

use std::fmt::Write;

use super::{
    CrateDefs,
    REGISTERED,
    RegisteredStep,
    current_crate_id,
    get_step_span,
    handle_validation_result,
    resolve_keywords,
};
use crate::{StepKeyword, parsing::feature::ParsedStep};

/// Validate steps against the exact lexical libraries selected by a scenario.
///
/// Unknown libraries can come from another crate or an out-of-line module, so
/// runtime lookup remains authoritative in that case.
pub(crate) fn validate_steps_exist_in_scope(
    steps: &[ParsedStep],
    libraries: &[Box<str>],
    strict: bool,
) -> Result<(), syn::Error> {
    let reg = REGISTERED
        .lock()
        .unwrap_or_else(std::sync::PoisonError::into_inner);
    let defs_owned = reg.get(current_crate_id()).cloned();
    let Some(defs) = defs_owned.as_ref() else {
        return Ok(());
    };
    if !defs.knows_all_libraries(libraries) {
        return Ok(());
    }
    drop(reg);
    let missing = steps
        .iter()
        .zip(resolve_keywords(steps))
        .map(|(step, keyword)| validate_scoped_step(step, keyword, defs, libraries))
        .collect::<Result<Vec<_>, _>>()?
        .into_iter()
        .flatten()
        .collect::<Vec<_>>();
    handle_validation_result(&missing, strict)
}

/// Validate one step against locally visible definitions in the selected scope.
fn validate_scoped_step(
    step: &ParsedStep,
    keyword: StepKeyword,
    defs: &CrateDefs,
    libraries: &[Box<str>],
) -> Result<Option<(proc_macro2::Span, String)>, syn::Error> {
    let matches = defs
        .scoped_patterns(keyword, libraries)
        .into_iter()
        .filter(|definition| {
            definition
                .pattern
                .captures(get_step_span(step), step.text.as_str())
                .is_some()
        })
        .collect::<Vec<_>>();
    match matches.len() {
        0 => Ok(Some((
            get_step_span(step),
            format_scoped_missing_step_error(step, keyword, defs, libraries),
        ))),
        1 => Ok(None),
        _ => Err(format_scoped_ambiguity_error(
            step, keyword, libraries, &matches,
        )),
    }
}

/// Format a missing-step diagnostic that explains the closed selected scope.
fn format_scoped_missing_step_error(
    step: &ParsedStep,
    keyword: StepKeyword,
    defs: &CrateDefs,
    libraries: &[Box<str>],
) -> String {
    let selected = format_library_list(libraries);
    let mut message = format!(
        "No matching step definition found for '{} {}'. Selected libraries: [{selected}]",
        keyword.as_str(),
        step.text,
    );
    let candidates = defs.matching_unselected(keyword, step, libraries);
    if !candidates.is_empty() {
        message.push_str("\nMatching definitions exist only in unselected libraries:");
        for candidate in candidates {
            let _ = write!(
                message,
                "\n  - {}: '{}' ({})",
                candidate.library,
                candidate.pattern.as_str(),
                candidate.source_location,
            );
        }
        let _ = write!(
            message,
            "\nAdd the required library to libraries = [{selected}, …]."
        );
    }
    message
}

/// Format ambiguity without allowing selected-library order to decide.
fn format_scoped_ambiguity_error(
    step: &ParsedStep,
    keyword: StepKeyword,
    libraries: &[Box<str>],
    candidates: &[&RegisteredStep],
) -> syn::Error {
    let selected = format_library_list(libraries);
    let mut message = format!(
        "Ambiguous step definition for '{} {}'. Selected libraries: [{selected}]",
        keyword.as_str(),
        step.text,
    );
    for candidate in candidates {
        let _ = write!(
            message,
            "\n  - {}: '{}' ({})",
            candidate.library,
            candidate.pattern.as_str(),
            candidate.source_location,
        );
    }
    syn::Error::new(get_step_span(step), message)
}

/// Render selected libraries in declaration order for diagnostics.
fn format_library_list(libraries: &[Box<str>]) -> String {
    libraries
        .iter()
        .map(AsRef::as_ref)
        .collect::<Vec<_>>()
        .join(", ")
}
