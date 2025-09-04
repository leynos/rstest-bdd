use super::*;
use rstest::rstest;
use serial_test::serial;

fn clear_registry() {
    REGISTERED
        .lock()
        .unwrap_or_else(|e| panic!("step registry poisoned: {e}"))
        .clear();
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
    assert!(err.contains("Matches: a step"));
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
