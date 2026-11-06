//! Compatibility tests for the deprecated registry lookup projections.
//!
//! Every deprecated helper must resolve the same registered step and project
//! the same handler, mode, and metadata that the canonical metadata lookups
//! return. The handlers registered here finish with a distinctive skipped
//! outcome, so a projection that returned the wrong function pointer would
//! fail an assertion instead of quietly returning `Continue`.

use rstest::rstest;
use rstest_bdd::{
    AsyncStepFn,
    PatternStr,
    Step,
    StepContext,
    StepError,
    StepExecution,
    StepExecutionMode,
    StepFn,
    StepFuture,
    StepKeyword,
    StepText,
    find_step,
    find_step_async,
    find_step_async_with_mode,
    find_step_with_metadata,
    find_step_with_mode,
    lookup_step,
    lookup_step_async,
    lookup_step_async_with_mode,
    step,
};

use crate::{
    noop_steps::{noop_async_wrapper, noop_wrapper},
    poll_step_future_support::poll_step_future,
};

/// Marker returned by the distinctive synchronous projection handler.
const SYNC_MARKER: &str = "sync-projection-marker";

/// Marker returned by the distinctive asynchronous projection handler.
const ASYNC_MARKER: &str = "async-projection-marker";

/// Step text registered for the distinctive projection handlers.
const DISTINCTIVE_STEP: &str = "a distinctive projection step";

/// Synchronous handler that finishes with a distinctive skipped outcome.
#[expect(
    clippy::unnecessary_wraps,
    reason = "handler must match the StepFn signature"
)]
fn distinctive_sync_handler(
    _ctx: &mut StepContext<'_>,
    _text: &str,
    _docstring: Option<&str>,
    _table: Option<&[&[&str]]>,
) -> Result<StepExecution, StepError> {
    Ok(StepExecution::skipped(SYNC_MARKER.to_owned()))
}

/// Asynchronous handler that finishes with a distinctive skipped outcome.
fn distinctive_async_handler<'ctx>(
    _ctx: &'ctx mut StepContext<'_>,
    _text: &'ctx str,
    _docstring: Option<&'ctx str>,
    _table: Option<&'ctx [&'ctx [&'ctx str]]>,
) -> StepFuture<'ctx> {
    Box::pin(std::future::ready(Ok(StepExecution::skipped(
        ASYNC_MARKER.to_owned(),
    ))))
}

step!(
    StepKeyword::Given,
    DISTINCTIVE_STEP,
    distinctive_sync_handler,
    distinctive_async_handler,
    &[]
);

// Register steps that pin a non-`Both` execution mode so the mode projection
// is observable rather than defaulted.
step!(
    StepKeyword::Given,
    "a sync-mode projection step",
    noop_wrapper,
    noop_async_wrapper,
    &[],
    mode = StepExecutionMode::Sync
);

step!(
    StepKeyword::Given,
    "an async-mode projection step",
    noop_wrapper,
    noop_async_wrapper,
    &[],
    mode = StepExecutionMode::Async
);

// Register a placeholder pattern so the fuzzy lookups have to match text that
// differs from the registered pattern.
step!(
    StepKeyword::Given,
    "a fuzzy projection step with {value}",
    noop_wrapper,
    noop_async_wrapper,
    &[]
);

/// Which deprecated synchronous projection a case exercises.
#[derive(Clone, Copy, Debug)]
enum SyncLookup {
    /// Exact-pattern projection, `lookup_step`.
    Exact,
    /// Fuzzy-pattern projection, `find_step`.
    Fuzzy,
    /// Metadata alias, `find_step_with_mode`, projecting `&'static Step`.
    MetadataAlias,
}

impl SyncLookup {
    /// Resolve the distinctive step through this projection.
    fn resolve(self) -> Option<StepFn> {
        let keyword = StepKeyword::Given;
        match self {
            Self::Exact => lookup_step(keyword, DISTINCTIVE_STEP.into()),
            Self::Fuzzy => find_step(keyword, DISTINCTIVE_STEP.into()),
            Self::MetadataAlias => {
                find_step_with_mode(keyword, DISTINCTIVE_STEP.into()).map(|step| step.run)
            }
        }
    }
}

/// Assert that a step outcome is the distinctive skipped outcome for `marker`.
fn assert_distinctive_skip(result: StepExecution, marker: &str) {
    match result {
        StepExecution::Skipped { message } => {
            assert_eq!(message.as_deref(), Some(marker));
        }
        other @ StepExecution::Continue { .. } => {
            panic!("expected a skipped outcome, got {other:?}")
        }
    }
}

/// Poll an async projection to completion with a default context.
fn poll_projection(handler: AsyncStepFn) -> StepExecution {
    let mut ctx = StepContext::default();
    let future = handler(&mut ctx, DISTINCTIVE_STEP, None, None);
    poll_step_future(future)
}

#[rstest]
#[case::exact(SyncLookup::Exact)]
#[case::fuzzy(SyncLookup::Fuzzy)]
#[case::metadata_alias(SyncLookup::MetadataAlias)]
fn deprecated_sync_lookup_projects_a_distinctive_handler(#[case] lookup: SyncLookup) {
    let handler = lookup.resolve().expect("step should be found");
    let mut ctx = StepContext::default();
    let result = handler(&mut ctx, DISTINCTIVE_STEP, None, None)
        .expect("projected sync handler should succeed");
    assert_distinctive_skip(result, SYNC_MARKER);
}

#[rstest]
#[case::exact(true)]
#[case::fuzzy(false)]
fn deprecated_async_lookup_projects_a_distinctive_handler(#[case] should_match_exactly: bool) {
    let handler = if should_match_exactly {
        lookup_step_async(StepKeyword::Given, DISTINCTIVE_STEP.into())
    } else {
        find_step_async(StepKeyword::Given, DISTINCTIVE_STEP.into())
    }
    .expect("step should be found");

    assert_distinctive_skip(poll_projection(handler), ASYNC_MARKER);
}

#[rstest]
#[case::exact(true)]
#[case::fuzzy(false)]
fn deprecated_mode_lookup_projects_a_distinctive_handler(#[case] should_match_exactly: bool) {
    let (handler, mode) = if should_match_exactly {
        lookup_step_async_with_mode(StepKeyword::Given, DISTINCTIVE_STEP.into())
    } else {
        find_step_async_with_mode(StepKeyword::Given, DISTINCTIVE_STEP.into())
    }
    .expect("step should be found");

    assert_eq!(mode, StepExecutionMode::Both);
    assert_distinctive_skip(poll_projection(handler), ASYNC_MARKER);
}

#[rstest]
#[case::sync("a sync-mode projection step", StepExecutionMode::Sync)]
#[case::async_mode("an async-mode projection step", StepExecutionMode::Async)]
fn deprecated_mode_lookup_reports_the_registered_mode(
    #[case] text: &str,
    #[case] expected: StepExecutionMode,
) {
    let exact = lookup_step_async_with_mode(StepKeyword::Given, PatternStr::from(text))
        .expect("exact lookup should find the step");
    let fuzzy = find_step_async_with_mode(StepKeyword::Given, StepText::from(text))
        .expect("fuzzy lookup should find the step");

    assert_eq!(exact.1, expected);
    assert_eq!(fuzzy.1, expected);
}

#[test]
fn find_step_with_mode_returns_the_registered_step_for_fuzzy_text() {
    let text = "a fuzzy projection step with 42";
    let resolved: &'static Step = find_step_with_mode(StepKeyword::Given, StepText::from(text))
        .expect("fuzzy lookup should find the step");

    assert_eq!(
        resolved.pattern.as_str(),
        "a fuzzy projection step with {value}"
    );
    assert_eq!(resolved.keyword, StepKeyword::Given);

    let metadata = find_step_with_metadata(StepKeyword::Given, StepText::from(text))
        .expect("metadata lookup should find the step");
    assert_eq!(
        metadata.as_step().pattern.as_str(),
        resolved.pattern.as_str()
    );
    assert_eq!(metadata.as_step().keyword, resolved.keyword);
}
