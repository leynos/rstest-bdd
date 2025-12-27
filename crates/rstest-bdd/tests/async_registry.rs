//! Tests for async step registry infrastructure.
//!
//! These tests verify that the async step registry correctly stores and
//! retrieves async step wrappers, and that sync steps are properly normalised
//! into the async interface. Tests also verify correct failure behaviour when
//! patterns or keywords do not match, and that async lookups properly mark
//! steps as used.

use rstest::rstest;
use rstest_bdd::{
    AsyncStepFn, Step, StepContext, StepExecution, StepFuture, StepKeyword, find_step_async, iter,
    lookup_step_async, step, unused_steps,
};

mod common;
use common::{noop_async_wrapper, noop_wrapper, poll_step_future};

// ----------------------------------------------------------------------------
// Test helper functions
// ----------------------------------------------------------------------------

/// Verify that an async step wrapper lookup succeeds and can be polled to completion.
#[expect(clippy::expect_used, reason = "test helper validates lookup succeeds")]
fn assert_async_wrapper_works(lookup_fn: impl FnOnce() -> Option<AsyncStepFn>, test_text: &str) {
    let async_fn = lookup_fn().expect("step should be found");
    let mut ctx = StepContext::default();
    let future = async_fn(&mut ctx, test_text, None, None);
    let result = poll_step_future(future);
    assert!(
        matches!(result, StepExecution::Continue { .. }),
        "unexpected result: {result:?}"
    );
}

/// Verify that a step is marked as used after being looked up.
fn assert_step_marked_as_used(
    pattern: &str,
    lookup_fn: impl FnOnce() -> Option<AsyncStepFn>,
    api_name: &str,
) {
    // Verify the step is initially in the unused list.
    let unused_before: Vec<_> = unused_steps().iter().map(|s| s.pattern.as_str()).collect();
    assert!(
        unused_before.contains(&pattern),
        "Step should initially appear in unused_steps"
    );

    // Resolve the step.
    let result = lookup_fn();
    assert!(result.is_some(), "Step should be found");

    // Verify the step is no longer in the unused list.
    let unused_after: Vec<_> = unused_steps().iter().map(|s| s.pattern.as_str()).collect();
    assert!(
        !unused_after.contains(&pattern),
        "Step should no longer appear in unused_steps after {api_name}"
    );
}

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
fn find_step_async_returns_async_wrapper() {
    assert_async_wrapper_works(
        || find_step_async(StepKeyword::Given, "an async registry test step".into()),
        "an async registry test step",
    );
}

#[test]
fn lookup_step_async_returns_async_wrapper() {
    assert_async_wrapper_works(
        || lookup_step_async(StepKeyword::Given, "an async registry test step".into()),
        "an async registry test step",
    );
}

// ----------------------------------------------------------------------------
// Parameterized tests for async lookup failure behaviour
// ----------------------------------------------------------------------------

/// Test that async lookup APIs return None when the pattern or keyword does not match.
///
/// This parameterized test consolidates all failure cases for both `find_step_async`
/// and `lookup_step_async` into a single test with multiple cases.
#[rstest]
#[case::find_unknown_pattern(
    "find_step_async",
    StepKeyword::Given,
    "a completely unknown pattern xyz123",
    "for an unknown pattern"
)]
#[case::find_mismatched_when(
    "find_step_async",
    StepKeyword::When,
    "an async registry test step",
    "when keyword does not match (When)"
)]
#[case::find_mismatched_then(
    "find_step_async",
    StepKeyword::Then,
    "an async registry test step",
    "when keyword does not match (Then)"
)]
#[case::lookup_unknown_pattern(
    "lookup_step_async",
    StepKeyword::Given,
    "a completely unknown pattern xyz123",
    "for an unknown pattern"
)]
#[case::lookup_mismatched_when(
    "lookup_step_async",
    StepKeyword::When,
    "an async registry test step",
    "when keyword does not match (When)"
)]
#[case::lookup_mismatched_then(
    "lookup_step_async",
    StepKeyword::Then,
    "an async registry test step",
    "when keyword does not match (Then)"
)]
fn async_lookup_returns_none_for_invalid_input(
    #[case] api_name: &str,
    #[case] keyword: StepKeyword,
    #[case] pattern: &str,
    #[case] failure_reason: &str,
) {
    let result = match api_name {
        "find_step_async" => find_step_async(keyword, pattern.into()),
        "lookup_step_async" => lookup_step_async(keyword, pattern.into()),
        _ => panic!("unknown API: {api_name}"),
    };
    assert!(
        result.is_none(),
        "{api_name} should return None {failure_reason}"
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
    assert_step_marked_as_used(
        "async unused tracking test step",
        || find_step_async(StepKeyword::Given, "async unused tracking test step".into()),
        "find_step_async",
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
    assert_step_marked_as_used(
        "async lookup unused tracking test step",
        || {
            lookup_step_async(
                StepKeyword::When,
                "async lookup unused tracking test step".into(),
            )
        },
        "lookup_step_async",
    );
}
