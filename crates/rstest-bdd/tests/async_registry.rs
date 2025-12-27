//! Tests for async step registry infrastructure.
//!
//! These tests verify that the async step registry correctly stores and
//! retrieves async step wrappers, and that sync steps are properly normalised
//! into the async interface. Tests also verify correct failure behaviour when
//! patterns or keywords do not match, and that async lookups properly mark
//! steps as used.

use rstest_bdd::{
    AsyncStepFn, Step, StepContext, StepExecution, StepFuture, StepKeyword, find_step_async, iter,
    lookup_step_async, step, unused_steps,
};

mod common;
use common::{noop_async_wrapper, noop_wrapper, poll_step_future};

// Register a test step for async registry tests.
step!(
    StepKeyword::Given,
    "an async registry test step",
    noop_wrapper,
    noop_async_wrapper,
    &[]
);

#[test]
fn async_step_fn_can_be_stored_and_invoked() {
    fn test_step<'a>(
        _ctx: &'a mut StepContext<'a>,
        _text: &str,
        _docstring: Option<&str>,
        _table: Option<&[&[&str]]>,
    ) -> StepFuture<'a> {
        Box::pin(std::future::ready(Ok(StepExecution::from_value(None))))
    }

    let step_fn: AsyncStepFn = test_step;
    let mut ctx = StepContext::default();
    let future = step_fn(&mut ctx, "test", None, None);
    let result = poll_step_future(future);
    assert!(
        matches!(result, StepExecution::Continue { .. }),
        "unexpected result: {result:?}"
    );
}

#[test]
#[expect(
    clippy::expect_used,
    reason = "test asserts Option is Some before expect"
)]
fn step_struct_has_run_async_field() {
    let found = iter::<Step>
        .into_iter()
        .find(|step| step.pattern.as_str() == "an async registry test step");

    assert!(found.is_some(), "test step should be registered");
    let step = found.expect("step found");

    // Verify that run_async is callable.
    let mut ctx = StepContext::default();
    let future = (step.run_async)(&mut ctx, "test", None, None);
    let result = poll_step_future(future);
    assert!(
        matches!(result, StepExecution::Continue { .. }),
        "unexpected result: {result:?}"
    );
}

#[test]
#[expect(clippy::expect_used, reason = "test validates step lookup succeeds")]
fn find_step_async_returns_async_wrapper() {
    let async_fn = find_step_async(StepKeyword::Given, "an async registry test step".into())
        .expect("step should be found");

    let mut ctx = StepContext::default();
    let future = async_fn(&mut ctx, "an async registry test step", None, None);
    let result = poll_step_future(future);
    assert!(
        matches!(result, StepExecution::Continue { .. }),
        "unexpected result: {result:?}"
    );
}

#[test]
#[expect(clippy::expect_used, reason = "test validates step lookup succeeds")]
fn lookup_step_async_returns_async_wrapper() {
    let async_fn = lookup_step_async(StepKeyword::Given, "an async registry test step".into())
        .expect("step should be found");

    let mut ctx = StepContext::default();
    let future = async_fn(&mut ctx, "an async registry test step", None, None);
    let result = poll_step_future(future);
    assert!(
        matches!(result, StepExecution::Continue { .. }),
        "unexpected result: {result:?}"
    );
}

// ----------------------------------------------------------------------------
// Tests for async lookup failure behaviour
// ----------------------------------------------------------------------------

#[test]
fn find_step_async_returns_none_for_unknown_pattern() {
    let result = find_step_async(
        StepKeyword::Given,
        "a completely unknown pattern xyz123".into(),
    );
    assert!(
        result.is_none(),
        "find_step_async should return None for an unknown pattern"
    );
}

#[test]
fn find_step_async_returns_none_for_mismatched_keyword() {
    // The registered step uses StepKeyword::Given, so When/Then should not match.
    let using_when = find_step_async(StepKeyword::When, "an async registry test step".into());
    let using_postcondition =
        find_step_async(StepKeyword::Then, "an async registry test step".into());

    assert!(
        using_when.is_none(),
        "find_step_async should return None when keyword does not match (When)"
    );
    assert!(
        using_postcondition.is_none(),
        "find_step_async should return None when keyword does not match (Then)"
    );
}

#[test]
fn lookup_step_async_returns_none_for_unknown_pattern() {
    let result = lookup_step_async(
        StepKeyword::Given,
        "a completely unknown pattern xyz123".into(),
    );
    assert!(
        result.is_none(),
        "lookup_step_async should return None for an unknown pattern"
    );
}

#[test]
fn lookup_step_async_returns_none_for_mismatched_keyword() {
    // The registered step uses StepKeyword::Given, so When/Then should not match.
    let using_when = lookup_step_async(StepKeyword::When, "an async registry test step".into());
    let using_postcondition =
        lookup_step_async(StepKeyword::Then, "an async registry test step".into());

    assert!(
        using_when.is_none(),
        "lookup_step_async should return None when keyword does not match (When)"
    );
    assert!(
        using_postcondition.is_none(),
        "lookup_step_async should return None when keyword does not match (Then)"
    );
}

// ----------------------------------------------------------------------------
// Tests for unused step tracking with async APIs
// ----------------------------------------------------------------------------

// Register a dedicated step for testing unused_steps() behaviour with async
// lookups. This step has a unique pattern to avoid conflicts with other tests.
step!(
    StepKeyword::Given,
    "async unused tracking test step",
    noop_wrapper,
    noop_async_wrapper,
    &[]
);

#[test]
fn find_step_async_marks_step_as_used() {
    // Verify the step is initially in the unused list.
    let unused_before: Vec<_> = unused_steps().iter().map(|s| s.pattern.as_str()).collect();
    assert!(
        unused_before.contains(&"async unused tracking test step"),
        "Step should initially appear in unused_steps"
    );

    // Resolve the step via the async API.
    let result = find_step_async(StepKeyword::Given, "async unused tracking test step".into());
    assert!(result.is_some(), "Step should be found");

    // Verify the step is no longer in the unused list.
    let unused_after: Vec<_> = unused_steps().iter().map(|s| s.pattern.as_str()).collect();
    assert!(
        !unused_after.contains(&"async unused tracking test step"),
        "Step should no longer appear in unused_steps after find_step_async"
    );
}

// Register another step for testing lookup_step_async unused tracking.
step!(
    StepKeyword::When,
    "async lookup unused tracking test step",
    noop_wrapper,
    noop_async_wrapper,
    &[]
);

#[test]
fn lookup_step_async_marks_step_as_used() {
    // Verify the step is initially in the unused list.
    let unused_before: Vec<_> = unused_steps().iter().map(|s| s.pattern.as_str()).collect();
    assert!(
        unused_before.contains(&"async lookup unused tracking test step"),
        "Step should initially appear in unused_steps"
    );

    // Resolve the step via the async lookup API.
    let result = lookup_step_async(
        StepKeyword::When,
        "async lookup unused tracking test step".into(),
    );
    assert!(result.is_some(), "Step should be found");

    // Verify the step is no longer in the unused list.
    let unused_after: Vec<_> = unused_steps().iter().map(|s| s.pattern.as_str()).collect();
    assert!(
        !unused_after.contains(&"async lookup unused tracking test step"),
        "Step should no longer appear in unused_steps after lookup_step_async"
    );
}
