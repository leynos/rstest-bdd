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
    pattern: String,
}

static REGISTERED: LazyLock<Mutex<Vec<RegisteredStep>>> = LazyLock::new(|| Mutex::new(Vec::new()));

/// Record a step definition so scenarios can validate against it.
pub(crate) fn register_step(keyword: StepKeyword, pattern: &syn::LitStr) {
    let mut reg = REGISTERED
        .lock()
        .unwrap_or_else(std::sync::PoisonError::into_inner);
    reg.push(RegisteredStep {
        keyword,
        pattern: pattern.value(),
    });
}

/// Ensure all parsed steps have matching definitions.
///
/// In strict mode, missing steps cause compilation to fail. In non-strict mode,
/// the function emits warnings but allows compilation to continue so scenarios
/// can reference steps from other crates. Ambiguous step definitions always
/// produce an error.
///
/// # Errors
/// Returns a `syn::Error` when `strict` is `true` and a step lacks a matching
/// definition or when any step matches more than one definition.
pub(crate) fn validate_steps_exist(steps: &[ParsedStep], strict: bool) -> Result<(), syn::Error> {
    let reg = REGISTERED
        .lock()
        .unwrap_or_else(std::sync::PoisonError::into_inner);
    let mut prev = None;
    let mut missing = Vec::new();
    for step in steps {
        let resolved = resolve_conjunction_keyword(&mut prev, step.keyword);
        if let Some(msg) = has_matching_step_definition(&reg, resolved, step)? {
            missing.push(msg);
        }
    }

    if missing.is_empty() {
        return Ok(());
    }

    if strict {
        let msg = if missing.len() == 1 {
            missing.remove(0)
        } else {
            missing.join("\n")
        };
        Err(syn::Error::new(proc_macro2::Span::call_site(), msg))
    } else {
        for msg in missing {
            #[expect(clippy::print_stderr, reason = "proc_macro::Diagnostic is unstable")]
            {
                eprintln!("warning: {msg} (will be checked at runtime)");
            }
        }
        Ok(())
    }
}

fn has_matching_step_definition(
    reg: &[RegisteredStep],
    resolved: StepKeyword,
    step: &ParsedStep,
) -> Result<Option<String>, syn::Error> {
    let matches: Vec<&RegisteredStep> = reg
        .iter()
        .filter(|def| step_matches_definition(def, resolved, step))
        .collect();

    if matches.is_empty() {
        let available_defs: Vec<&str> = reg
            .iter()
            .filter(|def| def.keyword == resolved)
            .map(|def| def.pattern.as_str())
            .collect();

        let possible_matches: Vec<&str> = available_defs
            .iter()
            .copied()
            .filter(|pattern| step.text.contains(pattern) || pattern.contains(&step.text))
            .collect();

        let mut msg = format!(
            "No matching step definition found for: {} {}",
            fmt_keyword(resolved),
            step.text
        );
        if !available_defs.is_empty() {
            msg.push('\n');
            msg.push_str("Available step definitions for this keyword:\n");
            for def in &available_defs {
                msg.push_str("  - ");
                msg.push_str(def);
                msg.push('\n');
            }
        }
        if !possible_matches.is_empty() {
            msg.push('\n');
            msg.push_str("Possible matches:\n");
            for m in &possible_matches {
                msg.push_str("  - ");
                msg.push_str(m);
                msg.push('\n');
            }
        }
        return Ok(Some(msg));
    } else if matches.len() > 1 {
        let patterns: Vec<&str> = matches.iter().map(|def| def.pattern.as_str()).collect();
        let msg = format!(
            "Ambiguous step definition for '{}'. Matches: {}",
            step.text,
            patterns.join(", ")
        );
        return Err(syn::Error::new(proc_macro2::Span::call_site(), msg));
    }
    Ok(None)
}

fn step_matches_definition(def: &RegisteredStep, resolved: StepKeyword, step: &ParsedStep) -> bool {
    if def.keyword != resolved {
        return false;
    }
    // Leak a clone of the pattern string to satisfy the `'static` lifetime
    // expected by `StepPattern::new`. The leak is acceptable because macros run
    // in a short-lived compiler process.
    let leaked: &'static str = Box::leak(def.pattern.clone().into_boxed_str());
    let pattern = StepPattern::new(leaked);
    extract_placeholders(&pattern, StepText::from(step.text.as_str())).is_ok()
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

#[cfg(test)]
mod tests {
    use super::*;
    use rstest::rstest;

    fn clear_registry() {
        REGISTERED
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner)
            .clear();
    }

    #[rstest]
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
        }];
        assert!(validate_steps_exist(&steps, true).is_ok());
        assert!(validate_steps_exist(&steps, false).is_ok());
    }

    #[rstest]
    fn errors_when_missing_step_in_strict_mode() {
        clear_registry();
        let steps = [ParsedStep {
            keyword: StepKeyword::Given,
            text: "missing".to_string(),
            docstring: None,
            table: None,
        }];
        assert!(validate_steps_exist(&steps, true).is_err());
        assert!(validate_steps_exist(&steps, false).is_ok());
    }

    #[rstest]
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
        }];
        let err = match validate_steps_exist(&steps, false) {
            Err(e) => e.to_string(),
            Ok(()) => panic!("expected ambiguous step error"),
        };
        assert!(err.contains("Ambiguous step definition"));
        assert!(validate_steps_exist(&steps, true).is_err());
    }
}
