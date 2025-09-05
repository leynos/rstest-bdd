//! Tests for step-definition validation: missing/single/ambiguous outcomes and registry behaviour.
use super::*;
use rstest::rstest;
use serial_test::serial;

#[expect(clippy::expect_used, reason = "registry lock must panic if poisoned")]
fn clear_registry() {
    REGISTERED.lock().expect("step registry poisoned").clear();
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
#[case::basic("a step", "a step")]
#[case::placeholder("I have {item}", "I have apples")]
#[case::typed("number {n:u32}", "number 42")]
#[serial]
fn validates_step_patterns(#[case] pattern: &str, #[case] test_text: &str) {
    clear_registry();
    register_step(
        StepKeyword::Given,
        &syn::LitStr::new(pattern, proc_macro2::Span::call_site()),
    );
    let steps = [create_test_step(test_text)];
    assert!(validate_steps_exist(&steps, true).is_ok());
    assert!(validate_steps_exist(&steps, false).is_ok());
}

#[rstest]
#[case::missing_step(None, "missing")]
#[case::foreign_crate_step(Some(("a step", "other")), "a step")]
#[serial]
fn validates_strict_mode_errors(
    #[case] foreign_step: Option<(&str, &str)>,
    #[case] step_text: &str,
) {
    clear_registry();
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
    clear_registry();
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
    clear_registry();
    // proc-macro-error panics outside macro contexts; just assert it aborts
    let result = std::panic::catch_unwind(|| {
        register_step(
            StepKeyword::Given,
            &syn::LitStr::new("unclosed {", proc_macro2::Span::call_site()),
        );
    });
    assert!(result.is_err());
}

// Additional unit coverage: exercise matcher outcomes directly without
// allocating a vector of matches, ensuring short-circuit behaviour.
#[derive(Debug, PartialEq, Eq)]
enum MatchOutcome {
    Missing,
    Single,
    Ambiguous,
}

#[expect(
    clippy::expect_fun_call,
    clippy::expect_used,
    reason = "test helper should panic with explicit message"
)]
fn make_step_pattern(src: &str) -> &'static StepPattern {
    let leaked: &'static str = Box::leak(src.to_string().into_boxed_str());
    let pattern: &'static StepPattern = Box::leak(Box::new(StepPattern::new(leaked)));
    pattern
        .compile()
        .expect(&format!("compile pattern '{}'", pattern.as_str()));
    pattern
}

fn make_defs_for(kw: StepKeyword, patterns: Vec<&str>) -> CrateDefs {
    let mut defs = CrateDefs::default();
    let list = defs.by_kw.entry(kw).or_default();
    for p in patterns {
        list.push(make_step_pattern(p));
    }
    defs
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
    let defs = make_defs_for(StepKeyword::Given, patterns);
    let step = create_test_step(text);
    // Ok(None) => exactly one match; Ok(Some(_)) => missing; Err(_) => ambiguous.
    let outcome = match has_matching_step_definition(&defs, StepKeyword::Given, &step) {
        Ok(Some(_)) => MatchOutcome::Missing,
        Ok(None) => MatchOutcome::Single,
        Err(_) => MatchOutcome::Ambiguous,
    };
    assert_eq!(outcome, expected, "unexpected outcome for text: {text}");
}
