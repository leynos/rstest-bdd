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
use crate::parsing::feature::{ParsedStep, resolve_conjunction_keyword};
use rstest_bdd::{StepPattern, StepText, extract_placeholders};

#[derive(Clone)]
struct RegisteredStep {
    keyword: StepKeyword,
    pattern: &'static str,
    crate_id: Box<str>,
}

static REGISTERED: LazyLock<Mutex<Vec<RegisteredStep>>> = LazyLock::new(|| Mutex::new(Vec::new()));

/// Record a step definition so scenarios can validate against it.
pub(crate) fn register_step(keyword: StepKeyword, pattern: &syn::LitStr) {
    let mut reg = REGISTERED
        .lock()
        .unwrap_or_else(|e| panic!("step registry poisoned: {e}"));
    let leaked: &'static str = Box::leak(pattern.value().into_boxed_str());
    reg.push(RegisteredStep {
        keyword,
        pattern: leaked,
        crate_id: current_crate_id().into_boxed_str(),
    });
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
    let reg = REGISTERED
        .lock()
        .unwrap_or_else(|e| panic!("step registry poisoned: {e}"));
    let current = current_crate_id();
    let scoped: Vec<_> = reg
        .iter()
        .filter(|d| d.crate_id.as_ref() == current.as_str())
        .cloned()
        .collect();
    let missing = collect_missing_steps(&scoped, steps)?;
    handle_validation_result(&missing, strict)
}

fn collect_missing_steps(
    reg: &[RegisteredStep],
    steps: &[ParsedStep],
) -> Result<Vec<(proc_macro2::Span, String)>, syn::Error> {
    let mut prev = None;
    let mut missing = Vec::new();
    for step in steps {
        let resolved = resolve_conjunction_keyword(&mut prev, step.keyword);
        if let Some(msg) = has_matching_step_definition(reg, resolved, step)? {
            missing.push((step.span, msg));
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
            .map(|(_, m)| m.clone())
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
        #[expect(clippy::print_stderr, reason = "proc_macro::Diagnostic is unstable")]
        {
            eprintln!(
                "[rstest-bdd][non-strict] {msg}\n  --> line {} column {}\n",
                loc.line,
                loc.column + 1
            );
        }
    }
}

fn has_matching_step_definition(
    reg: &[RegisteredStep],
    resolved: StepKeyword,
    step: &ParsedStep,
) -> Result<Option<String>, syn::Error> {
    let matches = find_step_matches(reg, resolved, step);

    match matches.len() {
        0 => Ok(Some(format_missing_step_error(reg, resolved, step))),
        1 => Ok(None),
        _ => Err(format_ambiguous_step_error(&matches, step)),
    }
}

fn step_matches_definition(def: &RegisteredStep, resolved: StepKeyword, step: &ParsedStep) -> bool {
    if def.keyword != resolved {
        return false;
    }
    let pattern = StepPattern::new(def.pattern);
    extract_placeholders(&pattern, StepText::from(step.text.as_str())).is_ok()
}

fn find_step_matches<'a>(
    reg: &'a [RegisteredStep],
    resolved: StepKeyword,
    step: &ParsedStep,
) -> Vec<&'a RegisteredStep> {
    reg.iter()
        .filter(|def| step_matches_definition(def, resolved, step))
        .collect()
}

fn format_missing_step_error(
    reg: &[RegisteredStep],
    resolved: StepKeyword,
    step: &ParsedStep,
) -> String {
    let available_defs: Vec<&'static str> = reg
        .iter()
        .filter(|d| d.keyword == resolved)
        .map(|d| d.pattern)
        .collect();
    let possible_matches: Vec<&str> = available_defs
        .iter()
        .copied()
        .filter(|pat| step.text.contains(*pat) || pat.contains(step.text.as_str()))
        .collect();
    build_missing_step_message(resolved, step, &available_defs, &possible_matches)
}

fn format_ambiguous_step_error(matches: &[&RegisteredStep], step: &ParsedStep) -> syn::Error {
    let patterns: Vec<&str> = matches.iter().map(|def| def.pattern).collect();
    let msg = format!(
        "Ambiguous step definition for '{}'. Matches: {}",
        step.text,
        patterns.join(", ")
    );
    syn::Error::new(step.span, msg)
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

#[cfg(test)]
mod tests {
    use super::*;
    use rstest::rstest;
    use serial_test::serial;

    fn clear_registry() {
        REGISTERED
            .lock()
            .unwrap_or_else(|e| panic!("step registry poisoned: {e}"))
            .clear();
    }

    #[rstest]
    #[serial]
    fn validates_when_step_present() {
        clear_registry();
        register_step(
            StepKeyword::Given,
            &syn::LitStr::new("a step", proc_macro2::Span::call_site()),
        );
        let steps = [ParsedStep {
            keyword: StepKeyword::Given,
            text: "a step".to_string(),
            docstring: None,
            table: None,
            span: proc_macro2::Span::call_site(),
        }];
        assert!(validate_steps_exist(&steps, true).is_ok());
        assert!(validate_steps_exist(&steps, false).is_ok());
    }

    #[rstest]
    #[serial]
    fn errors_when_missing_step_in_strict_mode() {
        clear_registry();
        let steps = [ParsedStep {
            keyword: StepKeyword::Given,
            text: "missing".to_string(),
            docstring: None,
            table: None,
            span: proc_macro2::Span::call_site(),
        }];
        assert!(validate_steps_exist(&steps, true).is_err());
        assert!(validate_steps_exist(&steps, false).is_ok());
    }

    #[rstest]
    #[serial]
    fn errors_when_step_ambiguous() {
        clear_registry();
        let lit = syn::LitStr::new("a step", proc_macro2::Span::call_site());
        register_step(StepKeyword::Given, &lit);
        register_step(StepKeyword::Given, &lit);
        let steps = [ParsedStep {
            keyword: StepKeyword::Given,
            text: "a step".to_string(),
            docstring: None,
            table: None,
            span: proc_macro2::Span::call_site(),
        }];
        let err = match validate_steps_exist(&steps, false) {
            Err(e) => e.to_string(),
            Ok(()) => panic!("expected ambiguous step error"),
        };
        assert!(err.contains("Ambiguous step definition"));
        assert!(validate_steps_exist(&steps, true).is_err());
    }

    #[rstest]
    #[serial]
    fn ignores_steps_from_other_crates() {
        clear_registry();
        REGISTERED
            .lock()
            .unwrap_or_else(|e| panic!("step registry poisoned: {e}"))
            .push(RegisteredStep {
                keyword: StepKeyword::Given,
                pattern: "a step",
                crate_id: "other".into(),
            });
        let steps = [ParsedStep {
            keyword: StepKeyword::Given,
            text: "a step".to_string(),
            docstring: None,
            table: None,
            span: proc_macro2::Span::call_site(),
        }];
        assert!(validate_steps_exist(&steps, true).is_err());
    }
}
