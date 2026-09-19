//! The INV-10 cases themselves, split out to stay inside the 400-line cap.
//!
//! Two cases, and both are needed. [`cancelling_during_a_step_drops_it_and_still_cleans_the_scope`]
//! asserts what cancellation does; [`a_run_left_alone_completes_and_produces_an_outcome`]
//! is the *positive* control that makes the first one evidence. Without it, every
//! assertion the cancellation case makes would pass for a driver that never
//! reached the awaiting position at all — the case would be describing an
//! unreachable state rather than a canceled one.
//!
//! The split follows the repository's convention for integration targets: a
//! Cargo test target is one `.rs` file, so a colocated module is reached with
//! `#[path = "..."]` from the parent. See `runner_panics.rs`.
//!
//! # Why the cleanup assertion reads the context and not the fixture cell
//!
//! The obvious way to observe cleanup is to hold the fixture's `RefCell` and
//! check it afterwards, because the fixture outlives the run's borrow of the
//! context. **That assertion is vacuous**, and it was written that way first.
//! `insert_value` writes the returned value into the context's *override map*
//! (`ctx.values`) rather than into the fixture's own cell, so the fixture cell
//! reads the same before and after cleanup. It would report "cleanup ran" for a
//! `CleanupGuard::drop` that did nothing at all.
//!
//! The discriminating read is `ctx.try_borrow::<Marker>(MARKER)`, which consults
//! the override map first and the fixture second: it yields the returned `7`
//! while the override is live and the fixture's own `0` once cleanup has cleared
//! it. The catch is that the live run holds `ctx` mutably, so that read is only
//! possible after the run has been dropped — which is exactly when the answer
//! becomes interesting. Binding the run inside a block and reading after it is
//! what makes the assertion both compile and mean something.

use std::{
    cell::RefCell,
    task::{Context, Poll, Waker},
};

use rstest_bdd::{
    StepContext,
    runner::{ScenarioScope, ScenarioStatus, run_scenario_async},
};

use crate::{
    MARKER, Marker, completing_plan, gate_drops, gate_polls, observed_before_cancel, plan,
    poll_until_gate_entered,
};

/// INV-10: dropping the future drops the in-flight step, and cleans the scope.
///
/// Asserts four things in the order that makes each meaningful: the run never
/// reached `Ready`; the gate *was* entered (or everything after is vacuous); the
/// gate had not been dropped while the run was alive; and it was dropped exactly
/// once after. Then, on the released context, that the override the run installed
/// is gone.
#[test]
fn cancelling_during_a_step_drops_it_and_still_cleans_the_scope() {
    // The fixture backing the override. Its own value is `0`, the returned one
    // is `7`, so the two are distinguishable in the post-cleanup read.
    let fixture = RefCell::new(Box::new(Marker(0)) as Box<dyn std::any::Any>);

    let mut ctx = StepContext::default();
    ctx.insert_owned::<Marker>(MARKER, &fixture);

    {
        let scope = ScenarioScope::new(&mut ctx);
        // The plan is bound rather than inlined: `run_scenario_async` borrows it,
        // so a temporary would be freed at the end of the `Box::pin` statement
        // while the run still held it.
        let plan = plan(true);
        let mut run = Box::pin(run_scenario_async(&plan, scope));

        let waker = Waker::noop();
        let mut cx = Context::from_waker(waker);
        let polls = poll_until_gate_entered(run.as_mut(), &mut cx);

        assert_eq!(
            gate_polls(),
            1,
            "the gate must have been polled exactly once after {polls} run polls, or the run \
             reached it more than once",
        );
        assert_eq!(
            observed_before_cancel(),
            Some(7),
            "the gate must have seen the returned override, not the fixture's own value: \
             otherwise this run had nothing for cleanup to clear and the assertion below would \
             hold under a driver that never cleaned up",
        );
        assert_eq!(
            gate_drops(),
            0,
            "the gate's drop probe fired before the run was dropped, so it cannot evidence that \
             the drop propagated",
        );

        drop(run);

        assert_eq!(
            gate_drops(),
            1,
            "dropping the pending run must drop the in-flight step future exactly once",
        );
    }

    // `ctx` is released here: the run that borrowed it has been dropped.
    assert_eq!(
        ctx.try_borrow::<Marker>(MARKER).map(|marker| marker.0).ok(),
        Some(0),
        "cancellation must still clear the step-returned override. Reading the fixture's own \
         value back means the override map was cleared; a `7` here would mean it survived",
    );
}

/// The normal-completion control: the same shape, run to `Ready`.
///
/// A plan that parks forever cannot complete, so this uses a plan whose single
/// invocation is an ordinary `Both`-mode passing step. What it holds constant is
/// the counter discipline: no gate was installed, so no drop probe may have
/// fired and no gate observation may have been recorded, and the outcome is a
/// real one. Together with the cancellation case's assertion that its own gate
/// *was* entered, that is what stops the cancellation assertions from describing
/// a position no run ever occupies.
#[test]
fn a_run_left_alone_completes_and_produces_an_outcome() {
    let mut ctx = StepContext::default();
    let plan = completing_plan();

    let scope = ScenarioScope::new(&mut ctx);
    let waker = Waker::noop();
    let mut cx = Context::from_waker(waker);
    let mut run = Box::pin(run_scenario_async(&plan, scope));

    let outcome = loop {
        match run.as_mut().poll(&mut cx) {
            Poll::Ready(outcome) => break outcome,
            Poll::Pending => continue,
        }
    };

    assert_eq!(
        outcome.status(),
        ScenarioStatus::Passed,
        "the control must complete normally; otherwise the cancellation cases describe a plan \
         that was never runnable",
    );
    assert_eq!(outcome.steps().len(), 1, "one invocation, one record");
    assert_eq!(
        gate_drops(),
        0,
        "no gate was installed in this case, so no drop probe may have fired",
    );
    assert_eq!(
        observed_before_cancel(),
        None,
        "no gate was installed in this case, so it must not have observed anything",
    );
}
