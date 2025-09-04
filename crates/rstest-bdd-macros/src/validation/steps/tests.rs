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
fn registry_cleared() {
    clear_registry();
}

#[rstest]
#[serial]
fn validates_when_step_present() {
    registry_cleared();
    register_step(
        StepKeyword::Given,
        &syn::LitStr::new("a step", proc_macro2::Span::call_site()),
    );
    let steps = [ParsedStep {
        keyword: StepKeyword::Given,
        text: "a step".to_string(),
        docstring: None,
        table: None,
        #[cfg(feature = "compile-time-validation")]
        span: proc_macro2::Span::call_site(),
    }];
    assert!(validate_steps_exist(&steps, true).is_ok());
    assert!(validate_steps_exist(&steps, false).is_ok());
}

#[rstest]
#[serial]
fn errors_when_missing_step_in_strict_mode() {
    registry_cleared();
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
    registry_cleared();
    let lit = syn::LitStr::new("a step", proc_macro2::Span::call_site());
    register_step(StepKeyword::Given, &lit);
    register_step(StepKeyword::Given, &lit);
    let steps = [ParsedStep {
        keyword: StepKeyword::Given,
        text: "a step".to_string(),
        docstring: None,
        table: None,
        #[cfg(feature = "compile-time-validation")]
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
    registry_cleared();
    register_step(
        StepKeyword::Given,
        &syn::LitStr::new("a step", proc_macro2::Span::call_site()),
    );
    let mut guard = REGISTERED
        .lock()
        .unwrap_or_else(|e| panic!("step registry poisoned: {e}"));
    if let Some(last) = guard.last_mut() {
        last.crate_id = "other".into();
    } else {
        panic!("registry empty");
    }
    let steps = [ParsedStep {
        keyword: StepKeyword::Given,
        text: "a step".to_string(),
        docstring: None,
        table: None,
        #[cfg(feature = "compile-time-validation")]
        span: proc_macro2::Span::call_site(),
    }];
    assert!(validate_steps_exist(&steps, true).is_err());
    assert!(validate_steps_exist(&steps, false).is_ok());
}
