//! Behavioural test for fixture context injection

use rstest_bdd::{StepContext, step};

fn needs_value(
    ctx: &StepContext<'_>,
    _text: &str,
    _docstring: Option<&str>,
    _table: Option<&[&[&str]]>,
) -> Result<(), String> {
    let val = ctx.get::<u32>("number").ok_or_else(|| {
        "Missing fixture 'number' of type 'u32' in step function 'needs_value'".to_string()
    })?;
    assert_eq!(*val, 42);
    Ok(())
}

step!(
    rstest_bdd::StepKeyword::Given,
    "a value",
    needs_value,
    &["number"]
);

#[test]
fn context_passes_fixture() {
    let number = 42u32;
    let mut ctx = StepContext::default();
    ctx.insert("number", &number);
    let step_fn = rstest_bdd::lookup_step(rstest_bdd::StepKeyword::Given, "a value".into())
        .unwrap_or_else(|| panic!("step 'a value' not found in registry"));
    let result = step_fn(&ctx, "a value", None, None);
    assert!(result.is_ok(), "step execution failed: {result:?}");
}

#[test]
fn context_missing_fixture_returns_error() {
    let ctx = StepContext::default();
    let step_fn = rstest_bdd::lookup_step(rstest_bdd::StepKeyword::Given, "a value".into())
        .unwrap_or_else(|| panic!("step 'a value' not found in registry"));
    let result = step_fn(&ctx, "a value", None, None);
    let err = match result {
        Ok(()) => panic!("expected error when fixture is missing"),
        Err(e) => e,
    };
    assert!(
        err.contains("Missing fixture 'number' of type 'u32'"),
        "unexpected error message"
    );
}
