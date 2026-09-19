//! The synchronous driver: resolve, execute, delegate, record.
//!
//! This file contains **no decision about a step result**. Every branch on
//! what a step did lives in [`policy`](super::policy); what is left here is the
//! loop, the registry call, the bookkeeping, and the one `match` that
//! [`classify`] already reduced. That is D6's claim, and it is the kind of claim
//! a reviewer cannot check by reading a diff, which is why
//! `runner_sequence_props.rs` and the `cargo-mutants` lane exist.
//!
//! The driver owns the two responsibilities the policy layer deliberately does
//! not: reaching the registry, and building the borrowed view a
//! [`StepExecutionRequest`] needs. Both touch process-global state or
//! allocate, which is exactly why they live here and not in `policy`.
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
    execution::{StepExecutionRequest, execute_step},
    runner::{
        ScenarioOutcome,
        ScenarioPlan,
        StepInvocation,
        ValueFate,
        engine::policy::{
            Absorbed,
            SkipPolicy,
            StepDecision,
            Terminal,
            absorb,
            assemble,
            classify,
        },
        outcome::StepOutcome,
    },
};

/// A borrowed rebuild of one invocation's data table.
///
/// Holds the per-row `Vec<&str>` that the request's inner slices point at.
/// Kept as its own type rather than a pair of locals so the rows cannot be
/// dropped while a view over them is alive.
struct TableView<'a> {
    /// One owned row each, borrowed from the plan.
    rows: Vec<Vec<&'a str>>,
}

impl<'a> TableView<'a> {
    /// Rebuild the plan's table as borrowed rows.
    ///
    /// `None` stays `None`, and `Some(empty)` stays `Some(empty)`: an invocation
    /// with no table and one with an empty table are different plans, and
    /// collapsing them here would make the request lie about which it was.
    fn new(table: Option<&'a [Vec<std::borrow::Cow<'static, str>>]>) -> Option<Self> {
        table.map(|rows| Self {
            rows: rows
                .iter()
                .map(|row| row.iter().map(std::borrow::Cow::as_ref).collect())
                .collect(),
        })
    }

    /// The outer view the request's `table` field takes.
    fn row_slices(&self) -> Vec<&[&str]> { self.rows.iter().map(Vec::as_slice).collect() }
}

/// Execute a plan against a context and assemble its outcome.
///
/// Resolves the skip policy from the scope's `fail_on_skipped` and the plan's
/// own `allow_skipped`, runs each invocation in plan order, records every one,
/// and hands the record plus the terminal event to [`assemble`].
///
/// # Panics
///
/// Does not panic. A step body that panics is turned into an ordinary failure
/// by the macro-generated wrapper's `catch_unwind`, which is the boundary this
/// driver relies on. Steps registered through a form without that wrapper are
/// not covered; see D11, which owns that gap.
pub(in crate::runner) fn drive(
    plan: &ScenarioPlan,
    ctx: &mut StepContext<'_>,
    fail_on_skipped: bool,
) -> ScenarioOutcome {
    let span = tracing::debug_span!("scenario", name = plan.name(), source = plan.source());
    let _entered = span.entered();

    // Resolved once, here: this is the first point at which both the plan's own
    // flag and the scope's resolved global are in hand. D10.
    let policy = SkipPolicy::resolve(plan.allow_skipped(), fail_on_skipped);
    tracing::debug!(
        allow_skipped = policy.allow_skipped,
        fail_on_skipped = policy.fail_on_skipped,
        forced_failure = policy.forces_failure(),
        "resolved skip policy",
    );

    let mut details = Vec::with_capacity(plan.steps().len());
    let mut terminal = None;
    let mut remaining = plan.steps().iter().enumerate();

    for (index, invocation) in remaining.by_ref() {
        let (record, stop) = execute(index, invocation, ctx, plan);
        details.push(record);
        if let Some(event) = stop {
            terminal = Some(event);
            break;
        }
    }

    // Every invocation after the terminal event is recorded and none of it is
    // executed. INV-2's completeness half; INV-1's termination half is the
    // `break` above.
    details.extend(remaining.map(|(index, invocation)| {
        StepOutcome::bypassed(
            index,
            invocation.keyword(),
            invocation.text(),
            invocation.source(),
        )
    }));

    assemble(details, terminal, policy)
}

/// Run one invocation and record what happened to it.
///
/// Returns this invocation's record and, when the run must stop, the event that
/// stopped it. The stop event is built here rather than in [`assemble`] because
/// the plan-side identity it needs — the index, and the invocation's own
/// source — is local to this loop.
fn execute(
    index: usize,
    invocation: &StepInvocation,
    ctx: &mut StepContext<'_>,
    plan: &ScenarioPlan,
) -> (StepOutcome, Option<Terminal>) {
    let table = TableView::new(invocation.table());
    let row_slices = table.as_ref().map(TableView::row_slices);
    let request = StepExecutionRequest {
        index,
        keyword: invocation.keyword(),
        text: invocation.text(),
        docstring: invocation.docstring(),
        table: row_slices.as_deref(),
        feature_path: plan.source(),
        scenario_name: plan.name(),
    };

    let result = execute_step(&request, ctx);
    // The insertion reaches `absorb` as a closure, so it happens exactly when a
    // value came back and necessarily before anything examines the error.
    let Absorbed { fate, error } = absorb(result, |value| ctx.insert_value(value).into());

    match classify(error) {
        StepDecision::Continue => (passed(index, invocation, fate), None),
        StepDecision::Skip { message } => {
            // D14: the terminal skip is logged with its identity and whether it
            // carried a reason — never with the reason itself, which is
            // step-supplied text.
            tracing::warn!(
                index,
                has_message = message.is_some(),
                "scenario stopped: a step requested a skip",
            );
            let record = StepOutcome::skipped(
                index,
                invocation.keyword(),
                invocation.text(),
                invocation.source(),
                message.clone(),
            );
            let terminal = Terminal::Skip {
                index,
                message,
                source: invocation.source().cloned(),
            };
            (record, Some(terminal))
        }
        StepDecision::Fail(error) => {
            // D14: the discriminant, never the formatted message.
            tracing::warn!(
                index,
                kind = ?crate::runner::FailureKind::of(&error),
                "scenario stopped: a step failed",
            );
            // One clone per scenario, on the failure path only: the record and
            // the terminal both need the error, and `StepOutcome::failed`
            // consumes its argument. See the plan's D18 note on the
            // alternatives that were rejected.
            let record = StepOutcome::failed(
                index,
                invocation.keyword(),
                invocation.text(),
                invocation.source(),
                error.clone(),
            );
            (record, Some(Terminal::Fail { index, error }))
        }
    }
}

/// Record a successful invocation, with what became of any returned value.
fn passed(index: usize, invocation: &StepInvocation, fate: Option<ValueFate>) -> StepOutcome {
    StepOutcome::passed(
        index,
        invocation.keyword(),
        invocation.text(),
        invocation.source(),
        fate,
    )
}
