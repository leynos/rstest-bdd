<<<<<<< HEAD
//! Tests for step validation: basic success, strict-mode errors, ambiguity and invalid patterns.
||||||| parent of e5b935e (Add module doc for step validation tests)
=======
//! Tests for step-definition validation: missing/single/ambiguous outcomes and registry behaviour.
>>>>>>> e5b935e (Add module doc for step validation tests)
use super::*;
use crate::StepKeyword;
use rstest::rstest;
use rstest_bdd::StepPattern;
use serial_test::serial;

#[expect(clippy::expect_used, reason = "registry lock must panic if poisoned")]
fn clear_registry() {
    REGISTERED.lock().expect("step registry poisoned").clear();
}

fn registry_cleared() {
    clear_registry();
}

fn create_test_step(text: &str) -> ParsedStep {
    ParsedStep {
        keyword: StepKeyword::Given,
        text: text.to_string(),
        docstring: None,
        table: None,
        #[cfg(feature = "compile-time-validation")]
        span: proc_macro2::Span::call_site(),
    }
}

#[rstest]
#[case("a step", "a step", "basic step")]
#[case("I have {item}", "I have apples", "placeholder step")]
#[case("number {n:u32}", "number 42", "typed placeholder")]
#[serial]
fn validates_step_patterns(
    #[case] pattern: &str,
    #[case] test_text: &str,
    #[case] description: &str,
) {
    registry_cleared();
    let _ = description;
    register_step(
        StepKeyword::Given,
        &syn::LitStr::new(pattern, proc_macro2::Span::call_site()),
    );
    let steps = [create_test_step(test_text)];
    assert!(validate_steps_exist(&steps, true).is_ok());
    assert!(validate_steps_exist(&steps, false).is_ok());
}

#[rstest]
#[case("missing_step", None, "missing")]
#[case("foreign_crate_step", Some(("a step", "other")), "a step")]
#[serial]
fn validates_strict_mode_errors(
    #[case] test_name: &str,
    #[case] foreign_step: Option<(&str, &str)>,
    #[case] step_text: &str,
) {
    registry_cleared();
    let _ = test_name;
    if let Some((pattern, crate_id)) = foreign_step {
        register_step_for_crate(StepKeyword::Given, pattern, crate_id);
    }
    let steps = [create_test_step(step_text)];
    assert!(validate_steps_exist(&steps, true).is_err());
    assert!(validate_steps_exist(&steps, false).is_ok());
}

#[rstest]
#[serial]
fn errors_when_step_ambiguous() {
    registry_cleared();
    let lit = syn::LitStr::new("a step", proc_macro2::Span::call_site());
    register_step(StepKeyword::Given, &lit);
    register_step(StepKeyword::Given, &lit);
    let steps = [create_test_step("a step")];
    let err = match validate_steps_exist(&steps, false) {
        Err(e) => e.to_string(),
        Ok(()) => panic!("expected ambiguous step error"),
    };
    assert!(err.contains("Ambiguous step definition"));
    assert!(err.contains("- a step"));
    // Count only lines that begin with the bullet, ignoring indented/reformatted lines.
    let bullet_count = err.lines().filter(|l| l.starts_with("- a step")).count();
    assert_eq!(bullet_count, 2, "expected two bullet matches");
    assert!(validate_steps_exist(&steps, true).is_err());
}

#[rstest]
#[serial]
fn aborts_on_invalid_step_pattern() {
    registry_cleared();
    // proc-macro-error panics outside macro contexts; just assert it aborts
    let result = std::panic::catch_unwind(|| {
        register_step(
            StepKeyword::Given,
            &syn::LitStr::new("unclosed {", proc_macro2::Span::call_site()),
        );
    });
    assert!(result.is_err());
}

#[derive(Debug, PartialEq, Eq)]
enum MatchOutcome {
    Missing,
    Single,
    Ambiguous,
}

/// Construct a `RegisteredStep` from a pattern for testing.
#[expect(
    clippy::expect_fun_call,
    clippy::expect_used,
    reason = "test helper should panic with explicit message"
)]
fn make_registered_step(src: &str) -> RegisteredStep {
    let leaked: &'static str = Box::leak(src.to_string().into_boxed_str());
    let pattern: &'static StepPattern = Box::leak(Box::new(StepPattern::new(leaked)));
    pattern
        .compile()
        .expect(&format!("compile pattern '{}'", pattern.as_str()));
    RegisteredStep {
        keyword: StepKeyword::Given,
        pattern,
        crate_id: "test".into(),
    }
}

/// Ensure the matcher distinguishes missing, unique, and ambiguous step definitions.
#[rstest]
#[case::missing(vec!["other"], "a step", MatchOutcome::Missing)]
#[case::single(vec!["a step"], "a step", MatchOutcome::Single)]
#[case::ambiguous(vec!["a {item}", "a step"], "a step", MatchOutcome::Ambiguous)]
fn has_matching_step_definition_cases(
    #[case] patterns: Vec<&str>,
    #[case] text: &str,
    #[case] expected: MatchOutcome,
) {
    let defs: Vec<RegisteredStep> = patterns.into_iter().map(make_registered_step).collect();
    let refs: Vec<&RegisteredStep> = defs.iter().collect();
    let step = create_test_step(text);
    // Ok(None) => exactly one match; Ok(Some(_)) => missing; Err(_) => ambiguous.
    let outcome = match has_matching_step_definition(&refs, StepKeyword::Given, &step) {
        Ok(Some(_)) => MatchOutcome::Missing,
        Ok(None) => MatchOutcome::Single,
        Err(_) => MatchOutcome::Ambiguous,
    };
    assert_eq!(outcome, expected, "unexpected outcome for text: {text}");
}
