//! Tests for step-definition validation: missing/single/ambiguous outcomes and registry behaviour.
use super::*;
use rstest::rstest;
use serial_test::serial;

#[expect(clippy::expect_used, reason = "registry lock must panic if poisoned")]
fn clear_registry() {
    REGISTERED.lock().expect("step registry poisoned").clear();
}

fn create_test_step(keyword: StepKeyword, text: &str) -> ParsedStep {
    ParsedStep {
        keyword,
        text: text.to_string(),
        docstring: None,
        table: None,
        #[cfg(feature = "compile-time-validation")]
        span: proc_macro2::Span::call_site(),
    }
}

fn assert_bullet_count(err: &str, expected: usize) {
    let bullet_count = err.lines().filter(|l| l.starts_with("- ")).count();
    assert_eq!(bullet_count, expected, "expected {expected} bullet matches");
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
    let steps = [create_test_step(StepKeyword::Given, test_text)];
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
    let steps = [create_test_step(StepKeyword::Given, step_text)];
    assert!(validate_steps_exist(&steps, true).is_err());
    assert!(validate_steps_exist(&steps, false).is_ok());
}

#[rstest]
#[case::literal("a step", "a step", "a step")]
#[case::placeholder("I have {item}", "I have {n:u32}", "I have 1")]
#[serial]
fn errors_when_step_ambiguous(
    #[case] pattern_a: &str,
    #[case] pattern_b: &str,
    #[case] text: &str,
) {
    clear_registry();
    let lit_a = syn::LitStr::new(pattern_a, proc_macro2::Span::call_site());
    let lit_b = syn::LitStr::new(pattern_b, proc_macro2::Span::call_site());
    register_step(StepKeyword::Given, &lit_a);
    register_step(StepKeyword::Given, &lit_b);
    let steps = [create_test_step(StepKeyword::Given, text)];
    let err = match validate_steps_exist(&steps, false) {
        Err(e) => e.to_string(),
        Ok(()) => panic!("expected ambiguous step error"),
    };
    assert!(err.contains("Ambiguous step definition"));
    assert!(err.contains(pattern_a));
    assert!(err.contains(pattern_b));
    assert_bullet_count(&err, 2);
    assert!(validate_steps_exist(&steps, true).is_err());
}

#[rstest]
#[serial]
fn aborts_on_invalid_step_pattern() {
    clear_registry();
    // proc-macro-error panics outside macro contexts; assert expected message
    let Err(err) = std::panic::catch_unwind(|| {
        register_step(
            StepKeyword::Given,
            &syn::LitStr::new("unclosed {", proc_macro2::Span::call_site()),
        );
    }) else {
        panic!("expected invalid step pattern to abort");
    };
    let msg = err
        .downcast_ref::<String>()
        .map(String::as_str)
        .or_else(|| err.downcast_ref::<&str>().copied())
        .unwrap_or_else(|| panic!("panic payload must be a string"));
    assert!(
        msg.contains("proc-macro-error API cannot be used outside of `entry_point` invocation")
    );

    assert!(rstest_bdd_patterns::build_regex_from_pattern("unclosed {").is_err());
}

#[test]
#[serial]
fn errors_when_step_matches_three_definitions() {
    clear_registry();
    let lit_a = syn::LitStr::new("I have {item}", proc_macro2::Span::call_site());
    let lit_b = syn::LitStr::new("I have {n:u32}", proc_macro2::Span::call_site());
    let lit_c = syn::LitStr::new("I have 1", proc_macro2::Span::call_site());
    register_step(StepKeyword::Given, &lit_a);
    register_step(StepKeyword::Given, &lit_b);
    register_step(StepKeyword::Given, &lit_c);
    let steps = [create_test_step(StepKeyword::Given, "I have 1")];
    let err = match validate_steps_exist(&steps, false) {
        Err(e) => e.to_string(),
        Ok(()) => panic!("expected ambiguous step error"),
    };
    assert!(err.contains("Ambiguous step definition"));
    assert!(err.contains("I have {item}"));
    assert!(err.contains("I have {n:u32}"));
    assert!(err.contains("I have 1"));
    assert_bullet_count(&err, 3);
    assert!(validate_steps_exist(&steps, true).is_err());
}
