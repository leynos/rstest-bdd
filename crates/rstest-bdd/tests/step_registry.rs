//! Behavioural test for step registry

use rstest::rstest;
use rstest_bdd::{Step, StepContext, StepError, StepKeyword, iter, step};

fn sample() {}
#[expect(
    clippy::unnecessary_wraps,
    reason = "wrapper must match StepFn signature"
)]
fn wrapper(
    ctx: &rstest_bdd::StepContext<'_>,
    _text: &str,
    _docstring: Option<&str>,
    _table: Option<&[&[&str]]>,
) -> Result<(), StepError> {
    // Adapter for zero-argument step functions
    let _ = ctx;
    sample();
    Ok(())
}

step!(rstest_bdd::StepKeyword::When, "behavioural", wrapper, &[]);

fn failing_wrapper(
    ctx: &StepContext<'_>,
    _text: &str,
    _docstring: Option<&str>,
    _table: Option<&[&[&str]]>,
) -> Result<(), StepError> {
    let _ = ctx;
    Err(StepError::ExecutionError {
        pattern: "fails".into(),
        function: "failing_wrapper".into(),
        message: "boom".into(),
    })
}

step!(
    rstest_bdd::StepKeyword::Given,
    "fails",
    failing_wrapper,
    &[]
);

#[expect(clippy::needless_pass_by_value, reason = "panic payload is dropped")]
fn extract_panic_message(e: Box<dyn std::any::Any + Send>) -> String {
    let any_ref = e.as_ref();

    macro_rules! try_downcast {
        ($($ty:ty),*) => {
            $(
                if let Some(val) = any_ref.downcast_ref::<$ty>() {
                    return val.to_string();
                }
            )*
        };
    }

    try_downcast!(&str, String, i32, u32, i64, u64, isize, usize, f32, f64);
    "non-string panic payload".to_string()
}

fn panicking_wrapper(
    ctx: &StepContext<'_>,
    _text: &str,
    _docstring: Option<&str>,
    _table: Option<&[&[&str]]>,
) -> Result<(), StepError> {
    use std::panic::{AssertUnwindSafe, catch_unwind};
    let _ = ctx;
    catch_unwind(AssertUnwindSafe(|| panic!("snap"))).map_err(|e| StepError::PanicError {
        pattern: "panics".into(),
        function: "panicking_wrapper".into(),
        message: extract_panic_message(e),
    })?;
    Ok(())
}

step!(
    rstest_bdd::StepKeyword::When,
    "panics",
    panicking_wrapper,
    &[]
);

#[test]
fn step_is_registered() {
    let found = iter::<Step>
        .into_iter()
        .any(|step| step.pattern.as_str() == "behavioural" && step.keyword == StepKeyword::When);
    assert!(found, "expected step not found");
}

#[rstest]
#[case(StepKeyword::Given, "fails", "failing_wrapper", "boom", true)]
#[case(StepKeyword::When, "panics", "panicking_wrapper", "snap", false)]
fn wrapper_error_handling(
    #[case] keyword: StepKeyword,
    #[case] pattern: &str,
    #[case] function_name: &str,
    #[case] expected_message: &str,
    #[case] is_execution_error: bool,
) {
    let step_fn = iter::<Step>
        .into_iter()
        .find(|s| s.pattern.as_str() == pattern && s.keyword == keyword)
        .map_or_else(
            || panic!("step '{pattern}' not found in registry"),
            |step| step.run,
        );
    let err = match step_fn(&StepContext::default(), pattern, None, None) {
        Ok(()) => panic!("expected error from wrapper '{pattern}'"),
        Err(e) => e,
    };
    match err {
        StepError::ExecutionError {
            pattern: p,
            function,
            message,
        } if is_execution_error => {
            assert_eq!(p, pattern);
            assert_eq!(function, function_name);
            assert_eq!(message, expected_message);
        }
        StepError::PanicError {
            pattern: p,
            function,
            message,
        } if !is_execution_error => {
            assert_eq!(p, pattern);
            assert_eq!(function, function_name);
            assert_eq!(message, expected_message);
        }
        other => panic!("unexpected error for '{pattern}': {other:?}"),
    }
}
