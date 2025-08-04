//! Behavioural test for fixture context injection

use rstest_bdd::{Step, StepContext, iter, step};

fn needs_value(ctx: &StepContext<'_>, _text: &str) -> Result<(), String> {
    let val = ctx
        .get::<u32>("number")
        .ok_or_else(|| "missing fixture".to_string())?;
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
    let result = step_fn(&ctx, "a value");
    assert!(result.is_ok(), "step execution failed: {result:?}");
}
