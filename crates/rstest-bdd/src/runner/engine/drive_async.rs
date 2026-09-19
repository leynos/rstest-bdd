//! The asynchronous driver: [`drive_sync`](super::drive_sync) with an `.await`.
//!
//! This file is deliberately almost a copy of its synchronous sibling, and the
//! near-duplication is the design rather than an oversight. D6 says the two
//! differ only in how they await a step; what can drift when that is written
//! down twice is *what they decide*, and every decision either loop makes is in
//! [`super::drive`] or in [`policy`](super::policy). What is left in each is a
//! `for` loop, an executor call, and a `break` — and of those three, only the
//! executor call differs.
//!
//! # Why the loop is not shared (D23)
//!
//! A single shared loop is possible: write it against a boxed next-poll future
//! and have each driver supply a poller, one returning an already-ready future
//! and one boxing [`execute_step_async`]. It was prototyped and it works. It
//! was rejected because [`run_scenario`](crate::runner::run_scenario) is a
//! plain synchronous `fn` returning a [`ScenarioOutcome`], so sharing the loop
//! would make that entry point depend on a poller that never yields — an
//! assumption with no type-level protection, and one for which this milestone
//! has no evidence: INV-5 compares *outcomes*, and a run that stopped early
//! because its sync poller yielded would differ loudly rather than subtly only
//! if the comparison happened to notice. Trading a compile-checked property of
//! the primary entry point for the removal of a duplication that INV-5 already
//! polices is the wrong way round.
//!
//! # The returned future is not `Send`
//!
//! It holds a `&mut StepContext`, which holds `RefCell`s and `Box<dyn Any>`
//! fixtures, so the future is `!Send` and a caller cannot `tokio::spawn` it.
//! That is inherited from the step execution path rather than chosen here — see
//! [`StepFuture`](crate::StepFuture)'s own note — and it is why the contract is
//! "current-thread or thread-per-scenario runtime" rather than "any runtime".
//! `crates/rstest-bdd/tests/runner_wire.rs` and the plan's D-notes record it as
//! a documented consequence rather than a limitation to be fixed.

use tracing::Instrument;

use crate::{
    StepContext,
    execution::execute_step_async,
    runner::{
        ScenarioOutcome,
        ScenarioPlan,
        engine::{
            drive::{TableView, bypassed, record_step, request},
            policy::{SkipPolicy, Terminal, assemble},
        },
        outcome::StepOutcome,
    },
};

/// Execute a plan against a context and assemble its outcome.
///
/// The asynchronous counterpart of [`drive_sync::drive`](super::drive_sync::drive),
/// and deliberately its shadow: it resolves the same skip policy, runs the same
/// loop, records through the same [`record_step`], and assembles through the
/// same [`assemble`]. The only difference is that a step is awaited.
///
/// # Cancellation
///
/// Dropping the returned future cancels the run: no outcome is produced, and
/// the in-flight step or hook future is dropped with it. Synchronous scope
/// cleanup still happens, because the caller's [`ScenarioScope`] is dropped by
/// whoever owns it — the scope is taken by value, so if the caller owns it then
/// dropping the future drops the scope with it.
///
/// [`ScenarioScope`]: crate::runner::ScenarioScope
///
/// # Panics
///
/// Does not panic, for the same reason as the synchronous driver:
/// [`execute_step_async`] catches a step body's unwind — per poll, since a
/// genuine `async` body may await real I/O — and returns it as a failure, so
/// this loop never sees one. A *destructor* that panics during cleanup is
/// caught by `runner::scope`'s `CleanupGuard`.
pub(in crate::runner) async fn drive(
    plan: &ScenarioPlan,
    ctx: &mut StepContext<'_>,
    fail_on_skipped: bool,
) -> ScenarioOutcome {
    let span = tracing::debug_span!(
        "scenario",
        name = plan.name(),
        source = plan.source(),
        line = plan.source_line(),
        steps = plan.steps().len(),
        allow_skipped = plan.allow_skipped(),
    );
    // `Instrument`, not `entered`. The span must be current for every event this
    // driver emits — including the per-step traces, which are emitted between
    // awaits — and an `Entered` guard is the wrong instrument for that: a guard
    // is thread-local and held across a suspension point, so it would report
    // this scenario as current on a thread that has since moved on to unrelated
    // work, and it would be dropped only when the future completed.
    // `Instrument` re-enters the span around each poll instead, which is both
    // correct under a work-stealing executor and the reason AGENTS.md forbids
    // the guard form. The synchronous sibling keeps `entered`, because its body
    // never suspends and so cannot observe the difference.
    //
    // The policy `debug!` below is emitted inside the instrumented future for
    // the same reason: emitted outside it, it would carry no scenario identity
    // and two concurrent runs would be indistinguishable in the log.
    drive_inner(plan, ctx, fail_on_skipped)
        .instrument(span)
        .await
}

/// The driver's body, run inside the scenario span.
///
/// Split from [`drive`] so the span can be attached to the whole future rather
/// than entered around a prefix of it. The split is the smallest one that makes
/// the instrumentation correct; the body is otherwise what [`drive`] was.
async fn drive_inner(
    plan: &ScenarioPlan,
    ctx: &mut StepContext<'_>,
    fail_on_skipped: bool,
) -> ScenarioOutcome {
    // Resolved once, here, exactly as the synchronous driver resolves it. Both
    // inputs and both outputs are logged together for the reason D14 gives: the
    // effective `allow_skipped` is derivable from the inputs, so a reader
    // comparing two runs needs the inputs to see *why* they differed.
    let policy = SkipPolicy::resolve(plan.allow_skipped(), fail_on_skipped);
    tracing::debug!(
        plan_allows_skipping = plan.allow_skipped(),
        fail_on_skipped = policy.fail_on_skipped(),
        allow_skipped = policy.allow_skipped(),
        forced_failure = policy.forced_failure(),
        "resolved skip policy",
    );

    let mut details = Vec::with_capacity(plan.steps().len());
    let mut terminal = None;
    let mut remaining = plan.steps().iter().enumerate();

    for (index, invocation) in remaining.by_ref() {
        let (record, stop) = execute(index, invocation, ctx, plan).await;
        tracing::trace!(
            index,
            keyword = ?invocation.keyword(),
            status = ?record.status(),
            "step executed",
        );
        details.push(record);
        if let Some(event) = stop {
            terminal = Some(event);
            break;
        }
    }

    details.extend(remaining.map(|(index, invocation)| bypassed(index, invocation)));

    assemble(details, terminal, policy)
}

/// Run one invocation asynchronously and record what happened to it.
///
/// The whole of this driver's difference from the synchronous one. The call it
/// makes runs a `StepExecutionMode::Async` handler's future to completion, and
/// for `Sync` or `Both` it calls the synchronous handler directly — which is
/// what makes a `Both`-registered plan produce identical outcomes through the
/// two drivers, and therefore what INV-5 asserts.
async fn execute(
    index: usize,
    invocation: &crate::runner::StepInvocation,
    ctx: &mut StepContext<'_>,
    plan: &ScenarioPlan,
) -> (StepOutcome, Option<Terminal>) {
    let table = TableView::new(invocation.table());
    let row_slices = table.as_ref().map(TableView::row_slices);
    let request = request(index, invocation, row_slices.as_deref(), plan);

    let result = execute_step_async(&request, ctx).await;
    record_step(index, invocation, ctx, result)
}
