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
}

static REGISTERED: LazyLock<Mutex<Vec<RegisteredStep>>> = LazyLock::new(|| Mutex::new(Vec::new()));

/// Record a step definition so scenarios can validate against it.
pub(crate) fn register_step(keyword: StepKeyword, pattern: &syn::LitStr) {
    // Leak the pattern string to obtain a `'static` lifetime; the small leak is
    // acceptable because macros run in a short-lived compiler process.
    let leaked: &'static str = Box::leak(pattern.value().into_boxed_str());
    let mut reg = REGISTERED
        .lock()
        .unwrap_or_else(std::sync::PoisonError::into_inner);
    reg.push(RegisteredStep {
        keyword,
        pattern: leaked,
    });
}

/// Ensure all parsed steps have matching definitions.
///
/// # Errors
/// Returns a `syn::Error` if a step lacks a corresponding definition.
pub(crate) fn validate_steps_exist(steps: &[ParsedStep]) -> Result<(), syn::Error> {
    let reg = REGISTERED
        .lock()
        .unwrap_or_else(std::sync::PoisonError::into_inner);
    let mut prev = None;
    'outer: for step in steps {
        let resolved = resolve_conjunction_keyword(&mut prev, step.keyword);
        for def in reg.iter() {
            if def.keyword == resolved {
                let pattern = StepPattern::new(def.pattern);
                if extract_placeholders(&pattern, StepText::from(step.text.as_str())).is_ok() {
                    continue 'outer;
                }
            }
        }
        let msg = format!(
            "step not found for: {} {}",
            fmt_keyword(resolved),
            step.text
        );
        return Err(syn::Error::new(proc_macro2::Span::call_site(), msg));
    }
    Ok(())
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
        assert!(validate_steps_exist(&steps).is_ok());
    }

    #[rstest]
    fn errors_when_missing_step() {
        clear_registry();
        let steps = [ParsedStep {
            keyword: StepKeyword::Given,
            text: "missing".to_string(),
            docstring: None,
            table: None,
        }];
        assert!(validate_steps_exist(&steps).is_err());
    }
}
