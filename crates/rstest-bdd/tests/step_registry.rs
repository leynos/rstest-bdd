//! Behavioural test for step registry

use rstest::rstest;
use rstest_bdd::localization::{strip_directional_isolates, ScopedLocalization};
use rstest_bdd::{
    iter, panic_message, step, Step, StepContext, StepError, StepExecution, StepKeyword,
};
use unic_langid::langid;

fn sample() {}
#[expect(
    clippy::unnecessary_wraps,
    reason = "wrapper must match StepFn signature"
)]
fn wrapper(
    ctx: &mut StepContext<'_>,
    _text: &str,
    _docstring: Option<&str>,
    _table: Option<&[&[&str]]>,
) -> Result<StepExecution, StepError> {
    // Adapter for zero-argument step functions
    let _ = ctx;
    sample();
    Ok(StepExecution::from_value(None))
}

step!(rstest_bdd::StepKeyword::When, "behavioural", wrapper, &[]);

fn failing_wrapper(
    ctx: &mut StepContext<'_>,
    _text: &str,
    _docstring: Option<&str>,
    _table: Option<&[&[&str]]>,
) -> Result<StepExecution, StepError> {
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

fn panicking_wrapper(
    ctx: &mut StepContext<'_>,
    _text: &str,
    _docstring: Option<&str>,
    _table: Option<&[&[&str]]>,
) -> Result<StepExecution, StepError> {
    use std::panic::{catch_unwind, AssertUnwindSafe};
    let _ = ctx;
    catch_unwind(AssertUnwindSafe(|| panic!("snap"))).map_err(|e| StepError::PanicError {
        pattern: "panics".into(),
        function: "panicking_wrapper".into(),
        message: panic_message(e.as_ref()),
    })?;
    Ok(StepExecution::from_value(None))
}

step!(
    rstest_bdd::StepKeyword::When,
    "panics",
    panicking_wrapper,
    &[]
);

fn needs_fixture_wrapper(
    ctx: &mut StepContext<'_>,
    _text: &str,
    _docstring: Option<&str>,
    _table: Option<&[&[&str]]>,
) -> Result<StepExecution, StepError> {
    if ctx.get::<u32>("missing").is_some() {
        Ok(StepExecution::from_value(None))
    } else {
        Err(StepError::MissingFixture {
            name: "missing".into(),
            ty: "u32".into(),
            step: "needs_fixture".into(),
        })
    }
}

step!(
    rstest_bdd::StepKeyword::Then,
    "needs fixture",
    needs_fixture_wrapper,
    &["missing"]
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
#[case(
    StepKeyword::Then,
    "needs fixture",
    "needs_fixture",
    "Missing fixture 'missing' of type 'u32' for step function 'needs_fixture'",
    true
)]
fn wrapper_handles_panic_and_non_panic_errors(
    #[case] keyword: StepKeyword,
    #[case] pattern: &str,
    #[case] function_name: &str,
    #[case] expected_message: &str,
    #[case] expects_non_panic_branch: bool,
) {
    let step_fn = iter::<Step>
        .into_iter()
        .find(|s| s.pattern.as_str() == pattern && s.keyword == keyword)
        .map_or_else(
            || panic!("step '{pattern}' not found in registry"),
            |step| step.run,
        );
    let mut ctx = StepContext::default();
    let Err(err) = step_fn(&mut ctx, pattern, None, None) else {
        panic!("expected error from wrapper '{pattern}'");
    };
    let err_display = strip_directional_isolates(&err.to_string());
    if expects_non_panic_branch {
        match err {
            StepError::ExecutionError {
                pattern: p,
                function,
                message,
            } => {
                assert_eq!(p, pattern);
                assert_eq!(function, function_name);
                assert_eq!(message, expected_message);
            }
            StepError::MissingFixture { name, ty, step } => {
                assert_eq!(step, function_name);
                assert_eq!(name, "missing");
                assert_eq!(ty, "u32");
                assert_eq!(err_display, expected_message);
            }
            other => panic!("unexpected error for '{pattern}': {other:?}"),
        }
    } else {
        match err {
            StepError::PanicError {
                pattern: p,
                function,
                message,
            } => {
                assert_eq!(p, pattern);
                assert_eq!(function, function_name);
                assert_eq!(message, expected_message);
            }
            other => panic!("unexpected error for '{pattern}': {other:?}"),
        }
    }
}

#[rstest]
#[case(StepKeyword::Given, "fails", "Erreur lors de l'exécution de l'étape")]
#[case(StepKeyword::When, "panics", "Panique dans l'étape")]
#[case(
    StepKeyword::Then,
    "needs fixture",
    "La fixture « missing » de type « u32 » est introuvable"
)]
fn wrapper_errors_localize(
    #[case] keyword: StepKeyword,
    #[case] pattern: &str,
    #[case] expected_snippet: &str,
) {
    let _guard = ScopedLocalization::new(&[langid!("fr")])
        .unwrap_or_else(|error| panic!("failed to scope French locale: {error}"));
    let step_fn = iter::<Step>
        .into_iter()
        .find(|s| s.pattern.as_str() == pattern && s.keyword == keyword)
        .map_or_else(
            || panic!("step '{pattern}' not found in registry"),
            |step| step.run,
        );
    let mut ctx = StepContext::default();
    let Err(err) = step_fn(&mut ctx, pattern, None, None) else {
        panic!("expected error from wrapper '{pattern}'");
    };
    let message = strip_directional_isolates(&err.to_string());
    assert!(
        message.contains(expected_snippet),
        "expected '{expected_snippet}' in '{message}'",
    );
}
