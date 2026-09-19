//! What both drivers share: the request view and the step-result record.
//!
//! D6 says the two drivers differ only in how they *await* a step. That is a
//! claim about the decisions they make, not about the text of their loops, and
//! this file is where the distinction is enforced: everything a driver would
//! otherwise decide twice lives here once. What remains in each driver is a
//! `for` loop whose body reads a request, calls its executor, and hands the
//! result to [`record_step`].
//!
//! # Why this is not the whole loop
//!
//! The first design put the *entire* loop here and parameterized it over a
//! boxed next-poll future, so that only one `for` existed anywhere. It was
//! withdrawn (D23): the synchronous entry point is a plain `fn` returning a
//! [`ScenarioOutcome`](crate::runner::ScenarioOutcome), so a shared loop would
//! have made that signature depend on a poller that never yields — an
//! assumption with no type-level protection and no evidence in this milestone
//! that would catch a silently truncated run. Sharing the *decisions* gets the
//! same protection against drift, because the part that could drift is the
//! part that is now shared.
//!
//! # The two responsibilities here
//!
//! [`request`] builds the borrowed view a
//! [`StepExecutionRequest`](crate::execution::StepExecutionRequest) needs, and
//! [`record_step`] turns one executor result into one
//! [`StepOutcome`](crate::runner::StepOutcome) plus, when the run must stop,
//! the [`Terminal`] that stopped it. Neither touches the registry or awaits
//! anything, so both are callable from either driver and from a unit test.

use std::{any::Any, borrow::Cow};

use crate::{
    StepContext,
    execution::{ExecutionError, StepExecutionRequest},
    runner::{
        ScenarioPlan,
        StepInvocation,
        ValueFate,
        engine::policy::{Absorbed, StepDecision, Terminal, absorb, classify},
        outcome::StepOutcome,
    },
};

/// A borrowed rebuild of one invocation's data table.
///
/// Holds the per-row `Vec<&str>` that the request's inner slices point at.
/// Kept as its own type rather than a pair of locals so the rows cannot be
/// dropped while a view over them is alive.
pub(super) struct TableView<'a> {
    /// One owned row each, borrowed from the plan.
    rows: Vec<Vec<&'a str>>,
}

impl<'a> TableView<'a> {
    /// Rebuild the plan's table as borrowed rows.
    ///
    /// `None` stays `None`, and `Some(empty)` stays `Some(empty)`: an invocation
    /// with no table and one with an empty table are different plans, and
    /// collapsing them here would make the request lie about which it was.
    pub(super) fn new(table: Option<&'a [Vec<Cow<'static, str>>]>) -> Option<Self> {
        table.map(|rows| Self {
            rows: rows
                .iter()
                .map(|row| row.iter().map(Cow::as_ref).collect())
                .collect(),
        })
    }

    /// The outer view the request's `table` field takes.
    pub(super) fn row_slices(&self) -> Vec<&[&str]> {
        self.rows.iter().map(Vec::as_slice).collect()
    }
}

/// Build the request one invocation is executed with.
///
/// The plan's `source` and `name` are the request's diagnostic context, and
/// they are the *plan's*, not the invocation's: a step's error message names
/// the scenario it belongs to, and `StepOutcome` carries the invocation's own
/// [`source`](StepInvocation::source) separately.
pub(super) fn request<'a>(
    index: usize,
    invocation: &'a StepInvocation,
    table: Option<&'a [&'a [&'a str]]>,
    plan: &'a ScenarioPlan,
) -> StepExecutionRequest<'a> {
    StepExecutionRequest {
        index,
        keyword: invocation.keyword(),
        text: invocation.text(),
        docstring: invocation.docstring(),
        table,
        feature_path: plan.source(),
        scenario_name: plan.name(),
    }
}

/// Record what happened to one invocation, and whether the run stops.
///
/// Takes the executor's result rather than producing it, which is the whole
/// point: `execute_step` and `execute_step_async` are the only things that
/// differ between the drivers, and this function cannot see which one ran.
///
/// Returns this invocation's record and, when the run must stop, the event that
/// stopped it. The stop event is built here rather than in
/// [`assemble`](super::policy::assemble) because the plan-side identity it
/// needs — the index, and the invocation's own source — is in hand here.
pub(super) fn record_step(
    index: usize,
    invocation: &StepInvocation,
    ctx: &mut StepContext<'_>,
    result: Result<Option<Box<dyn Any>>, ExecutionError>,
) -> (StepOutcome, Option<Terminal>) {
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
                location = location(invocation),
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
                location = location(invocation),
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

/// Record an invocation that followed the run's terminal event.
///
/// Every invocation past the terminal is recorded and none of it is executed.
/// INV-2's completeness half; INV-1's termination half is the `break` in each
/// driver's loop. Shared here rather than written twice because the two drivers
/// have no reason to differ on it at all.
pub(super) fn bypassed(index: usize, invocation: &StepInvocation) -> StepOutcome {
    tracing::trace!(
        index,
        keyword = ?invocation.keyword(),
        status = ?crate::runner::StepStatus::Bypassed,
        "step bypassed",
    );
    StepOutcome::bypassed(
        index,
        invocation.keyword(),
        invocation.text(),
        invocation.source(),
    )
}

/// Render a step's source as `path:line`, or `unknown` when the plan has none.
///
/// D14 asks the two terminal warnings to carry `path:line` rather than the
/// `SourceLocation` itself, because a `tracing` field is read in a log line and
/// the two coordinates are the whole of what a reader needs. `Display` is
/// deliberately not implemented on `SourceLocation` for this: the type carries
/// an optional column, and a `Display` would have to choose whether to render
/// it — a rendering decision that belongs to the log site, not to a
/// frontend-facing type that `runner/tests/surface.rs` also polices.
///
/// A plan may omit the location entirely (D3 makes it optional), and the
/// warning still has to be emitted for that run. `unknown` is used rather than
/// an empty string so the field is visibly present-but-absent rather than
/// looking like a rendering bug. Both arms build a `String` — the located arm
/// formats a path and a line into one, the unlocated arm copies the literal —
/// so the return type is owned rather than borrowed, and the caller can hand it
/// to the logging call without borrowing the invocation.
fn location(invocation: &StepInvocation) -> String {
    invocation.source().map_or_else(
        || "unknown".to_owned(),
        |source| format!("{}:{}", source.path(), source.line()),
    )
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
