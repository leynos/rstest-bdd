//! Tests for async step registry infrastructure.
//!
//! These tests verify that the async step registry correctly stores and
//! retrieves async step wrappers, and that sync steps are properly normalized
//! into the async interface. Tests also verify correct failure behaviour when
//! patterns or keywords do not match, and that async lookups properly mark
//! steps as used.

#![expect(
    deprecated,
    reason = "tests the deprecated registry lookup compatibility boundary"
)]

use rstest::rstest;
use rstest_bdd::{
    AsyncStepFn,
    ResolvedStep,
    Step,
    StepContext,
    StepExecution,
    StepExecutionMode,
    StepFuture,
    StepKeyword,
    find_step,
    find_step_async,
    find_step_async_with_mode,
    find_step_with_metadata,
    find_step_with_mode,
    iter,
    lookup_step,
    lookup_step_async,
    lookup_step_async_with_mode,
    lookup_step_with_metadata,
    step,
    unused_steps,
};

#[path = "common/noop_steps.rs"]
mod noop_steps;
#[path = "common/poll_step_future.rs"]
mod poll_step_future_support;
use noop_steps::{noop_async_wrapper, noop_wrapper};
use poll_step_future_support::poll_step_future;

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

/// Resolve a step through either canonical metadata lookup.
fn resolve_metadata_step(
    should_match_exactly: bool,
    keyword: StepKeyword,
    pattern: &str,
) -> Option<ResolvedStep> {
    if should_match_exactly {
        lookup_step_with_metadata(keyword, pattern.into())
    } else {
        find_step_with_metadata(keyword, pattern.into())
    }
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
    fn test_step<'ctx>(
        _ctx: &'ctx mut StepContext<'_>,
        _text: &'ctx str,
        _docstring: Option<&'ctx str>,
        _table: Option<&'ctx [&'ctx [&'ctx str]]>,
    ) -> StepFuture<'ctx> {
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

#[rstest]
#[case::fuzzy(false)]
#[case::exact(true)]
fn metadata_lookup_returns_async_wrapper(#[case] should_match_exactly: bool) {
    assert_async_wrapper_works(
        || {
            resolve_metadata_step(
                should_match_exactly,
                StepKeyword::Given,
                "an async registry test step",
            )
            .map(|step| step.run_async)
        },
        "an async registry test step",
    );
}

// ----------------------------------------------------------------------------
// Deprecated lookup compatibility tests
// ----------------------------------------------------------------------------

#[rstest]
#[case::exact(0)]
#[case::fuzzy(1)]
#[case::metadata_alias(2)]
fn deprecated_sync_lookup_projects_an_invocable_handler(#[case] variant: usize) {
    let handler = match variant {
        0 => lookup_step(StepKeyword::Given, "an async registry test step".into()),
        1 => find_step(StepKeyword::Given, "an async registry test step".into()),
        2 => {
            let resolved: Option<&'static Step> =
                find_step_with_mode(StepKeyword::Given, "an async registry test step".into());
            resolved.map(|step| step.run)
        }
        _ => panic!("unknown sync lookup variant: {variant}"),
    }
    .expect("step should be found");

    let mut ctx = StepContext::default();
    let result = handler(&mut ctx, "an async registry test step", None, None);
    assert!(
        matches!(result, Ok(StepExecution::Continue { .. })),
        "unexpected result: {result:?}"
    );
}

#[rstest]
#[case::exact(true)]
#[case::fuzzy(false)]
fn deprecated_async_lookup_projects_an_invocable_handler(#[case] should_match_exactly: bool) {
    assert_async_wrapper_works(
        || {
            if should_match_exactly {
                lookup_step_async(StepKeyword::Given, "an async registry test step".into())
            } else {
                find_step_async(StepKeyword::Given, "an async registry test step".into())
            }
        },
        "an async registry test step",
    );
}

#[rstest]
#[case::exact(true)]
#[case::fuzzy(false)]
fn deprecated_mode_lookup_projects_an_invocable_handler(#[case] should_match_exactly: bool) {
    let (handler, mode) = if should_match_exactly {
        lookup_step_async_with_mode(StepKeyword::Given, "an async registry test step".into())
    } else {
        find_step_async_with_mode(StepKeyword::Given, "an async registry test step".into())
    }
    .expect("step should be found");

    assert_eq!(mode, StepExecutionMode::Both);
    assert_async_wrapper_works(|| Some(handler), "an async registry test step");
}

// ----------------------------------------------------------------------------
// Parameterized tests for async lookup failure behaviour
// ----------------------------------------------------------------------------

/// Test that async lookup APIs return None when the pattern or keyword does not match.
///
/// This parameterized test consolidates all failure cases for both canonical
/// metadata lookups into a single test with multiple cases.
#[rstest]
#[case::find_unknown_pattern(
    "find_step_with_metadata",
    StepKeyword::Given,
    "a completely unknown pattern xyz123",
    "for an unknown pattern"
)]
#[case::find_mismatched_when(
    "find_step_with_metadata",
    StepKeyword::When,
    "an async registry test step",
    "when keyword does not match (When)"
)]
#[case::find_mismatched_then(
    "find_step_with_metadata",
    StepKeyword::Then,
    "an async registry test step",
    "when keyword does not match (Then)"
)]
#[case::lookup_unknown_pattern(
    "lookup_step_with_metadata",
    StepKeyword::Given,
    "a completely unknown pattern xyz123",
    "for an unknown pattern"
)]
#[case::lookup_mismatched_when(
    "lookup_step_with_metadata",
    StepKeyword::When,
    "an async registry test step",
    "when keyword does not match (When)"
)]
#[case::lookup_mismatched_then(
    "lookup_step_with_metadata",
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
    let result = resolve_metadata_step(api_name == "lookup_step_with_metadata", keyword, pattern)
        .map(|step| step.run_async);
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

step!(
    StepKeyword::When,
    "async lookup unused tracking test step",
    noop_wrapper,
    noop_async_wrapper,
    &[]
);

#[rstest]
#[case::fuzzy(
    false,
    StepKeyword::Given,
    "async unused tracking test step",
    "find_step_with_metadata"
)]
#[case::exact(
    true,
    StepKeyword::When,
    "async lookup unused tracking test step",
    "lookup_step_with_metadata"
)]
fn metadata_lookup_marks_step_as_used(
    #[case] should_match_exactly: bool,
    #[case] keyword: StepKeyword,
    #[case] pattern: &str,
    #[case] api_name: &str,
) {
    assert_step_marked_as_used(
        pattern,
        || resolve_metadata_step(should_match_exactly, keyword, pattern).map(|step| step.run_async),
        api_name,
    );
}
