//! Tests for async step registry infrastructure.
//!
//! These tests verify that the async step registry correctly stores and
//! retrieves async step wrappers, and that sync steps are properly normalised
//! into the async interface.

use rstest_bdd::{
    AsyncStepFn, Step, StepContext, StepExecution, StepFuture, StepKeyword, find_step_async, iter,
    lookup_step_async, step,
};

mod common;
use common::{noop_async_wrapper, noop_wrapper};

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

    // Poll the future to completion using a noop waker.
    let waker = std::task::Waker::noop();
    let mut cx = std::task::Context::from_waker(&waker);
    let mut pinned = future;
    match std::pin::Pin::as_mut(&mut pinned).poll(&mut cx) {
        std::task::Poll::Ready(Ok(StepExecution::Continue { .. })) => {}
        other => panic!("unexpected result: {other:?}"),
    }
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

    // Poll to completion.
    let waker = std::task::Waker::noop();
    let mut cx = std::task::Context::from_waker(&waker);
    let mut pinned = future;
    match std::pin::Pin::as_mut(&mut pinned).poll(&mut cx) {
        std::task::Poll::Ready(Ok(StepExecution::Continue { .. })) => {}
        other => panic!("unexpected result: {other:?}"),
    }
}

#[test]
fn find_step_async_returns_async_wrapper() {
    let async_fn =
        find_step_async(StepKeyword::Given, "an async registry test step".into())
            .expect("step should be found");

    let mut ctx = StepContext::default();
    let future = async_fn(&mut ctx, "an async registry test step", None, None);

    // Poll to completion.
    let waker = std::task::Waker::noop();
    let mut cx = std::task::Context::from_waker(&waker);
    let mut pinned = future;
    match std::pin::Pin::as_mut(&mut pinned).poll(&mut cx) {
        std::task::Poll::Ready(Ok(StepExecution::Continue { .. })) => {}
        other => panic!("unexpected result: {other:?}"),
    }
}

#[test]
fn lookup_step_async_returns_async_wrapper() {
    let async_fn = lookup_step_async(
        StepKeyword::Given,
        "an async registry test step".into(),
    )
    .expect("step should be found");

    let mut ctx = StepContext::default();
    let future = async_fn(&mut ctx, "an async registry test step", None, None);

    // Poll to completion.
    let waker = std::task::Waker::noop();
    let mut cx = std::task::Context::from_waker(&waker);
    let mut pinned = future;
    match std::pin::Pin::as_mut(&mut pinned).poll(&mut cx) {
        std::task::Poll::Ready(Ok(StepExecution::Continue { .. })) => {}
        other => panic!("unexpected result: {other:?}"),
    }
}
