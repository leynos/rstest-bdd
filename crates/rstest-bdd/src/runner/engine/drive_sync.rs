//! The synchronous driver: resolve, execute, record.
//!
//! This file contains **no decision about a step result**. Every branch on
//! what a step did lives either in [`policy`](super::policy) or in
//! [`drive`](super::drive); what is left here is a `for` loop, the registry
//! call, and the `break` that ends the run. That is D6's claim, and it is the
//! kind of claim a reviewer cannot check by reading a diff, which is why
//! `runner_sequence_props.rs` and the `cargo-mutants` lane exist.
//!
//! The driver owns the one responsibility the policy layer deliberately does
//! not: reaching the registry. That touches process-global state, which is
//! exactly why it lives here and not in `policy`.
//!
//! # What is shared with the async driver, and what is not
//!
//! The request view and the step-result record are in [`super::drive`],
//! because neither depends on how a step is executed. What is left here is the
//! executor call — [`execute_step`] — and its `.await`-free spelling;
//! [`super::drive_async`] is the same loop with `execute_step_async(...).await`
//! in that one position. See D23 for why the loop itself is not shared: writing
//! it against a boxed next-poll future would make this plain synchronous `fn`
//! depend on a poller that never yields, and nothing in the type system or in
//! this milestone's evidence would catch a run that silently stopped early.
//!
//! # Borrowing the request
//!
//! A [`StepExecutionRequest`] borrows its `text`, `docstring`, `table`, and
//! `feature_path` all under one lifetime. The plan holds the first three as
//! borrowed `Cow<'static, str>` and, for the table, as
//! `Vec<Vec<Cow<'static, str>>>` rather than the `&[&[&str]]` the request wants.
//!
//! So the table needs a borrowed rebuild on every call, and it needs *two*
//! levels of it: a `Vec<&str>` per row for the request's inner slice, and a
//! `Vec<&[&str]>` over those for its outer one. `Vec<T>` and `&[T]` do not
//! share a layout, so neither level can be reached by coercion. The
//! alternative — widening `StepExecutionRequest::table` to accept the plan's
//! shape — would change a type `rstest-bdd-macros` also constructs, which
//! Constraint 1 keeps out of scope for this milestone. The cost is two small
//! allocations per step that has a table, on the path where a step is being
//! executed anyway.

use crate::{
    StepContext,
    execution::execute_step,
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

/// Execute a plan synchronously and assemble its outcome.
///
/// Resolves the skip policy from the scope's `fail_on_skipped` and the plan's
/// own `allow_skipped`, runs each invocation in plan order, records every one,
/// and hands the record plus the terminal event to [`assemble`].
///
/// # Panics
///
/// Does not panic. A step body that panics is turned into an ordinary failure
/// before it reaches this loop: the macro-generated wrapper catches an
/// attribute-registered step's panic, and D11's boundary in
/// `execution::unwind` catches a raw `step!` handler's, which has no wrapper.
/// Either way [`execute_step`] returns, so this driver never sees an unwind
/// from a step. A *destructor* that panics during cleanup is a separate case
/// and does not pass through here at all; `runner::scope`'s `CleanupGuard`
/// owns it.
pub(in crate::runner) fn drive(
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
    let _entered = span.entered();

    // Resolved once, here: this is the first point at which both the plan's own
    // flag and the scope's resolved global are in hand. D10.
    let policy = SkipPolicy::resolve(plan.allow_skipped(), fail_on_skipped);
    // Both inputs are logged alongside both outputs, deliberately. The
    // effective `allow_skipped` is `plan_allows_skipping || !fail_on_skipped`,
    // so writing the inputs as well is what makes the resolution attributable
    // after the fact — the question D14 names ("why did my skip become a
    // failure on CI but not locally") is answered by seeing `fail_on_skipped`
    // differ between the two runs, and a reader does not have to re-derive
    // which input granted the flag.
    tracing::debug!(
        plan_allows_skipping = plan.allow_skipped(),
        fail_on_skipped = policy.fail_on_skipped,
        allow_skipped = policy.allow_skipped,
        forced_failure = policy.forces_failure(),
        "resolved skip policy",
    );

    let mut details = Vec::with_capacity(plan.steps().len());
    let mut terminal = None;
    let mut remaining = plan.steps().iter().enumerate();

    for (index, invocation) in remaining.by_ref() {
        let (record, stop) = execute(index, invocation, ctx, plan);
        // D14's per-step event, emitted from the loop rather than from
        // `execute`, so one call site covers all four statuses instead of one
        // per match arm. It follows the terminal `warn!` that `record_step` may
        // have just emitted for this same step: the terminal event is the
        // louder one and is announced first, and the trace then records what
        // that step's own entry was.
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

    // Every invocation after the terminal event is recorded and none of it is
    // executed. INV-2's completeness half; INV-1's termination half is the
    // `break` above.
    details.extend(remaining.map(|(index, invocation)| bypassed(index, invocation)));

    assemble(details, terminal, policy)
}

/// Run one invocation synchronously and record what happened to it.
///
/// The whole of this driver's difference from the asynchronous one is the call
/// in the middle; everything either side of it is [`super::drive`]'s.
fn execute(
    index: usize,
    invocation: &crate::runner::StepInvocation,
    ctx: &mut StepContext<'_>,
    plan: &ScenarioPlan,
) -> (StepOutcome, Option<Terminal>) {
    let table = TableView::new(invocation.table());
    let row_slices = table.as_ref().map(TableView::row_slices);
    let request = request(index, invocation, row_slices.as_deref(), plan);

    let result = execute_step(&request, ctx);
    record_step(index, invocation, ctx, result)
}
