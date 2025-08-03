//! Behavioural test for fixture context injection

use rstest_bdd::{Step, StepContext, StepError, iter, step};

fn needs_value(ctx: &StepContext<'_>, _text: &str) -> Result<(), StepError> {
    let val = ctx
        .get::<u32>("number")
        .ok_or(StepError::MissingFixture {
            name: "number",
            ty: "u32",
            step: "needs_value",
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
    let mut ctx = StepContext::default();
    let number = 42u32;
    ctx.insert("number", &number);
    let step_fn = iter::<Step>
        .into_iter()
        .find(|s| s.pattern.as_str() == "a value")
        .map_or_else(
            || panic!("step 'a value' not found in registry"),
            |step| step.run,
        );
    step_fn(&ctx, "a value").unwrap_or_else(|err| panic!("step failed: {err}"));
}

#[test]
fn needs_value_returns_error_when_fixture_missing() {
    let ctx = StepContext::default();
    let err = match needs_value(&ctx, "") {
        Ok(()) => panic!("expected missing fixture error"),
        Err(e) => e,
    };
    assert!(matches!(
        err,
        StepError::MissingFixture {
            name: "number",
            ty: "u32",
            step: "needs_value",
        }
    ));
}
