//! INV-15: an `Async`-mode step under the *synchronous* runner.
//!
//! `run_scenario` never consults `execution_mode`. What decides the outcome is
//! one layer down, in the macro-generated wrapper: it first asks
//! `tokio::runtime::Handle::try_current()`, and if a runtime is already current
//! it polls the step's future exactly once with a no-op waker, mapping
//! `Poll::Pending` to an `ExecutionError` whose message ends "multi-poll async
//! steps are not supported under a harness". Otherwise it builds a fresh
//! current-thread runtime and drives the step under a `LocalSet` to completion.
//!
//! The consequence a frontend needs to know is that the *same* step succeeds
//! outside a runtime and fails inside one, and that the difference is invisible
//! from `execute_step`. A frontend embedding `run_scenario` inside its own
//! runtime would otherwise learn this empirically, in production.
//!
//! # Non-vacuity
//!
//! "The outcomes differ" is satisfied by a witness that fails in *both*
//! positions, so both outcomes are asserted concretely rather than compared:
//! outside, the step passes and its three suspensions are counted; inside, it
//! fails with the diagnostic and *zero* suspensions were reached. The resume
//! count is what makes the step demonstrably multi-poll rather than merely
//! asynchronous — an `async fn` that never suspends would give the same status
//! in both positions, and [`a_non_suspending_async_step_passes_in_both_positions`]
//! is the control that separates "a runtime is current" from "the step
//! suspended".
//!
//! # Why this is an integration test
//!
//! D21: the case that carries the invariant must be *found in the registry*
//! before it can run, and the unit-test binary cannot reach the registry at all.

use std::cell::Cell;

use rstest::rstest;
use rstest_bdd::{
    StepContext,
    StepKeyword,
    runner::{
        ScenarioOutcome,
        ScenarioPlan,
        ScenarioPlanBuilder,
        ScenarioScope,
        ScenarioStatus,
        run_scenario,
    },
};
use rstest_bdd_macros::given;

thread_local! {
    /// How many times the suspending step has been *resumed*.
    ///
    /// Thread-local rather than a static because the two positions below run in
    /// the same process; a shared counter would let them race under a threaded
    /// test harness, and the value read would then belong to whichever run
    /// finished last.
    static RESUMES: Cell<u32> = const { Cell::new(0) };
}

/// An `Async`-mode step that suspends `times` times before finishing.
///
/// The counter is incremented *after* each yield, so it records resumptions
/// rather than suspensions attempted: a run that never gets past the first
/// pending poll leaves it at zero, which is precisely the in-runtime case.
#[given("an async-mode step that yields {times:u32} times")]
async fn an_async_mode_step_that_yields(times: u32) {
    for _ in 0..times {
        tokio::task::yield_now().await;
        RESUMES.with(|count| count.set(count.get() + 1));
    }
}

/// An `Async`-mode step that never suspends.
///
/// `StepExecutionMode::Async`, like its sibling above, because it is an
/// `async fn`. It is the control for the runtime-position claim: a step whose
/// first poll returns `Ready` cannot be affected by which position it runs in,
/// so if *this* one diverged, the cause would be the runtime and not the
/// suspension.
#[given("an async-mode step that does not yield")]
async fn an_async_mode_step_that_does_not_yield() {}

/// A one-step plan invoking `text`.
fn plan(text: &'static str) -> ScenarioPlan {
    ScenarioPlanBuilder::new("modes", "notes/modes.md")
        .step_at(StepKeyword::Given, text, 3)
        .build()
}

/// Run `text` with no runtime current, on this thread.
fn run_outside_a_runtime(text: &'static str) -> (ScenarioOutcome, u32) {
    RESUMES.with(|count| count.set(0));
    let mut ctx = StepContext::default();
    let scope = ScenarioScope::new(&mut ctx).with_skip_policy(false);
    let outcome = run_scenario(&plan(text), scope);
    (outcome, RESUMES.with(Cell::get))
}

/// Run `text` from inside a live current-thread runtime.
fn run_inside_a_runtime(text: &'static str) -> (ScenarioOutcome, u32) {
    RESUMES.with(|count| count.set(0));
    let runtime = tokio::runtime::Builder::new_current_thread()
        .enable_all()
        .build()
        .unwrap_or_else(|e| panic!("a current-thread runtime must be constructible: {e}"));
    let outcome = runtime.block_on(async {
        let mut ctx = StepContext::default();
        let scope = ScenarioScope::new(&mut ctx).with_skip_policy(false);
        run_scenario(&plan(text), scope)
    });
    (outcome, RESUMES.with(Cell::get))
}

/// The step's failure message, when the run failed with a handler error.
fn failure_message(outcome: &ScenarioOutcome) -> Option<String> {
    match outcome.failure()?.error()? {
        rstest_bdd::ExecutionError::HandlerFailed { error, .. } => match error.as_ref() {
            rstest_bdd::StepError::ExecutionError { message, .. } => Some(message.clone()),
            _ => None,
        },
        _ => None,
    }
}

/// INV-15's two positions, each asserted concretely.
#[rstest]
#[case::outside_a_runtime(false)]
#[case::inside_a_runtime(true)]
fn a_suspending_async_step_diverges_on_runtime_position(#[case] inside_a_runtime: bool) {
    let step = "an async-mode step that yields 3 times";
    let (outcome, resumes) = if inside_a_runtime {
        run_inside_a_runtime(step)
    } else {
        run_outside_a_runtime(step)
    };

    if inside_a_runtime {
        assert_eq!(
            outcome.status(),
            ScenarioStatus::Failed,
            "a runtime is already current, so the wrapper polls once; the step suspends and the \
             run fails rather than blocking: {outcome:?}",
        );
        assert_eq!(
            resumes, 0,
            "the step must not have been resumed even once — if it were, the wrapper would have \
             polled it more than once",
        );
        let message = failure_message(&outcome)
            .unwrap_or_else(|| panic!("the failure must be a handler error: {outcome:?}"));
        assert!(
            message.contains("multi-poll async steps are not supported under a harness"),
            "the diagnostic is the whole value of this case: a frontend seeing `Failed` must be \
             able to learn *why* from the message. Got: {message}",
        );
    } else {
        assert_eq!(
            outcome.status(),
            ScenarioStatus::Passed,
            "with no runtime current the wrapper builds one and drives the step to completion: \
             {outcome:?}",
        );
        assert_eq!(
            resumes, 3,
            "the step must have been resumed exactly as many times as it asked to yield; a lower \
             count would mean the run finished without suspending, and this case would no longer \
             witness multi-poll",
        );
    }
}

/// The control: a non-suspending `Async`-mode step passes in both positions.
///
/// Without this, `a_suspending_async_step_diverges_on_runtime_position` could
/// be read as "the macro path is unreachable from inside a runtime", which is a
/// stronger and false claim. Here the same registration form, found through the
/// same lookup and run through the same driver, succeeds either way; only
/// suspension changes the answer.
#[rstest]
#[case::outside_a_runtime(false)]
#[case::inside_a_runtime(true)]
fn a_non_suspending_async_step_passes_in_both_positions(#[case] inside_a_runtime: bool) {
    let step = "an async-mode step that does not yield";
    let (outcome, _) = if inside_a_runtime {
        run_inside_a_runtime(step)
    } else {
        run_outside_a_runtime(step)
    };

    assert_eq!(
        outcome.status(),
        ScenarioStatus::Passed,
        "a step whose first poll is `Ready` completes wherever it is polled: {outcome:?}",
    );
    assert!(
        outcome.failure().is_none(),
        "and so produces no failure for the position to have caused",
    );
}
