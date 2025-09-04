//! Compile-time step registration and validation.
//!
//! This module stores step definitions registered via `#[given]`, `#[when]`,
//! and `#[then]` attribute macros and provides validation utilities for the
//! `#[scenario]` macro. It ensures that every Gherkin step in a scenario has a
//! corresponding step definition. Missing steps yield a `compile_error!` during
//! macro expansion, preventing tests from compiling with incomplete behaviour
//! coverage.

use std::sync::{LazyLock, Mutex};

use crate::StepKeyword;
use crate::parsing::feature::ParsedStep;
use proc_macro_error::{abort, emit_warning};
use rstest_bdd::{StepPattern, extract_placeholders};

#[derive(Clone)]
struct RegisteredStep {
    keyword: StepKeyword,
    pattern: &'static StepPattern,
    // Regex compiled at registration to avoid repeat work.
    crate_id: Box<str>,
}

static REGISTERED: LazyLock<Mutex<Vec<RegisteredStep>>> = LazyLock::new(|| Mutex::new(Vec::new()));

/// Record a step definition so scenarios can validate against it.
pub(crate) fn register_step(keyword: StepKeyword, pattern: &syn::LitStr) {
    let leaked: &'static str = Box::leak(pattern.value().into_boxed_str());
    // Leak into `static`; step count is bounded during compile time.
    let step_pattern: &'static StepPattern = Box::leak(Box::new(StepPattern::new(leaked)));
    if let Err(e) = step_pattern.compile() {
        abort!(
            pattern.span(),
            "rstest-bdd-macros: Invalid step pattern '{}' in #[step] macro: {}",
            leaked,
            e
        );
    }
    let crate_id = current_crate_id().into_boxed_str();
    #[expect(
        clippy::expect_used,
        reason = "lock poisoning is unrecoverable; panic with clear message"
    )]
    let mut reg = REGISTERED.lock().expect("step registry poisoned");
    reg.push(RegisteredStep { keyword, pattern: step_pattern, crate_id });
}

/// Ensure all parsed steps have matching definitions.
///
/// In strict mode, missing steps cause compilation to fail. In non-strict mode,
/// the function emits warnings but allows compilation to continue so scenarios
/// can reference steps from other crates. Ambiguous step definitions within
/// this crate always produce an error.
///
/// # Errors
/// Returns a `syn::Error` when `strict` is `true` and a step lacks a matching
/// definition or when any step matches more than one definition.
pub(crate) fn validate_steps_exist(steps: &[ParsedStep], strict: bool) -> Result<(), syn::Error> {
    let current = current_crate_id();
    #[expect(
        clippy::expect_used,
        reason = "lock poisoning is unrecoverable; panic with clear message"
    )]
    let reg = REGISTERED.lock().expect("step registry poisoned");
    if !reg.iter().any(|d| d.crate_id.as_ref() == current.as_str()) && !strict {
        return Ok(());
    }
    let missing = collect_missing_steps(&reg, &current, steps)?;
    drop(reg);
    handle_validation_result(&missing, strict)
}

fn collect_missing_steps(
    reg: &[RegisteredStep],
    current: &str,
    steps: &[ParsedStep],
) -> Result<Vec<(proc_macro2::Span, String)>, syn::Error> {
    // Resolve conjunctions (And/But) deterministically to the preceding
    // primary keyword while preserving span-aware diagnostics.
    let resolved = resolve_keywords(steps);
    debug_assert_eq!(resolved.len(), steps.len());
    let mut missing = Vec::new();
    for (step, keyword) in steps.iter().zip(resolved) {
        if let Some(msg) = has_matching_step_definition(reg, current, keyword, step)? {
            let span = {
                #[cfg(feature = "compile-time-validation")]
                {
                    step.span
                }
                #[cfg(not(feature = "compile-time-validation"))]
                {
                    proc_macro2::Span::call_site()
                }
            };
            missing.push((span, msg));
        }
    }
    Ok(missing)
}

fn handle_validation_result(
    missing: &[(proc_macro2::Span, String)],
    strict: bool,
) -> Result<(), syn::Error> {
    if missing.is_empty() {
        return Ok(());
    }

    if strict {
        create_strict_mode_error(missing)
    } else {
        emit_non_strict_warnings(missing);
        Ok(())
    }
}

fn create_strict_mode_error(missing: &[(proc_macro2::Span, String)]) -> Result<(), syn::Error> {
    let msg = match missing {
        [(span, only)] => {
            return Err(syn::Error::new(*span, only.clone()));
        }
        _ => missing
            .iter()
            .map(|(_, m)| format!("• {m}"))
            .collect::<Vec<_>>()
            .join("\n"),
    };
    let span = missing
        .first()
        .map_or_else(proc_macro2::Span::call_site, |(s, _)| *s);
    Err(syn::Error::new(span, msg))
}

fn emit_non_strict_warnings(missing: &[(proc_macro2::Span, String)]) {
    for (span, msg) in missing {
        let loc = span.start();
        if loc.line == 0 && loc.column == 0 {
            emit_warning!(
                proc_macro2::Span::call_site(),
                "rstest-bdd[non-strict]: {}",
                msg;
                note = "location unavailable (synthetic or default span)"
            );
        } else {
            emit_warning!(*span, "rstest-bdd[non-strict]: {}", msg);
        }
    }
}

fn find_step_matches<'a>(
    defs: &'a [&RegisteredStep],
    step: &ParsedStep,
) -> Vec<&'a RegisteredStep> {
    defs.iter()
        .copied()
        .filter(|def| extract_placeholders(def.pattern, step.text.as_str().into()).is_ok())
        .collect()
}

fn has_matching_step_definition(
    reg: &[RegisteredStep],
    current: &str,
    resolved: StepKeyword,
    step: &ParsedStep,
) -> Result<Option<String>, syn::Error> {
    let defs: Vec<_> = reg
        .iter()
        .filter(|d| d.crate_id.as_ref() == current)
        .filter(|d| d.keyword == resolved)
        .collect();
    let matches = find_step_matches(&defs, step);

    match matches.len() {
        0 => Ok(Some(format_missing_step_error(
            reg, current, resolved, step,
        ))),
        1 => Ok(None),
        _ => Err(format_ambiguous_step_error(&matches, step)),
    }
}

fn format_missing_step_error(
    reg: &[RegisteredStep],
    current: &str,
    resolved: StepKeyword,
    step: &ParsedStep,
) -> String {
    let defs: Vec<&RegisteredStep> = reg
        .iter()
        .filter(|d| d.crate_id.as_ref() == current)
        .filter(|d| d.keyword == resolved)
        .collect();
    let available_defs: Vec<&'static str> = defs.iter().map(|d| d.pattern.as_str()).collect();
    let possible_matches: Vec<&str> = defs
        .iter()
        .filter(|d| d.pattern.regex().is_match(step.text.as_str()))
        .map(|d| d.pattern.as_str())
        .collect();
    build_missing_step_message(resolved, step, &available_defs, &possible_matches)
}

fn format_ambiguous_step_error(matches: &[&RegisteredStep], step: &ParsedStep) -> syn::Error {
    let patterns: Vec<&str> = matches.iter().map(|def| def.pattern.as_str()).collect();
    let msg = format!(
        "Ambiguous step definition for '{}'. Matches: {}",
        step.text,
        patterns.join(", ")
    );
    let span = {
        #[cfg(feature = "compile-time-validation")]
        {
            step.span
        }
        #[cfg(not(feature = "compile-time-validation"))]
        {
            proc_macro2::Span::call_site()
        }
    };
    syn::Error::new(span, msg)
}

fn build_missing_step_message(
    resolved: StepKeyword,
    step: &ParsedStep,
    available_defs: &[&str],
    possible_matches: &[&str],
) -> String {
    let mut msg = format!(
        "No matching step definition found for: {} {}",
        fmt_keyword(resolved),
        step.text
    );
    msg.push_str(&format_item_list(
        available_defs,
        "Available step definitions for this keyword:\n",
        |s| *s,
    ));
    msg.push_str(&format_item_list(
        possible_matches,
        "Possible matches:\n",
        |s| *s,
    ));
    msg
}

fn format_item_list<T, F>(items: &[T], header: &str, fmt_item: F) -> String
where
    F: Fn(&T) -> &str,
{
    if items.is_empty() {
        return String::new();
    }

    let mut msg = String::new();
    msg.push('\n');
    msg.push_str(header);
    for item in items {
        msg.push_str("  - ");
        msg.push_str(fmt_item(item));
        msg.push('\n');
    }
    msg
}

fn fmt_keyword(kw: StepKeyword) -> &'static str {
    match kw {
        StepKeyword::Given => "Given",
        StepKeyword::When => "When",
        StepKeyword::Then => "Then",
        StepKeyword::And => "And",
        StepKeyword::But => "But",
    }
}

fn current_crate_id() -> String {
    let name = std::env::var("CARGO_CRATE_NAME")
        .or_else(|_| std::env::var("CARGO_PKG_NAME"))
        .unwrap_or_else(|_| "unknown".to_owned());
    let out_dir = std::env::var("OUT_DIR").unwrap_or_default();
    format!("{name}:{out_dir}")
}

/// Resolve textual conjunctions ("And"/"But") to the semantic keyword of the
/// preceding step.
///
/// Seeds the chain with the first primary keyword, defaulting to `Given` when
/// none is found.
/// Returns an iterator yielding one keyword per input step.
pub(crate) fn resolve_keywords(
    steps: &[ParsedStep],
) -> impl ExactSizeIterator<Item = crate::StepKeyword> + '_ {
    let mut prev = steps
        .iter()
        .find_map(|s| match s.keyword {
            crate::StepKeyword::And | crate::StepKeyword::But => None,
            other => Some(other),
        })
        .or(Some(crate::StepKeyword::Given));
    let resolved = steps.iter().map(move |s| s.keyword.resolve(&mut prev));
    debug_assert_eq!(resolved.len(), steps.len());
    resolved
}
#[cfg(test)]
mod tests;
