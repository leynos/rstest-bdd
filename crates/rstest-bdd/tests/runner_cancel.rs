//! INV-10: dropping a pending `run_scenario_async` future cancels the run.
//!
//! Cancellation has three observable consequences and the cases in [`cases`]
//! assert all three, in the order that makes each one meaningful:
//!
//! 1. **No outcome is produced.** The future never returns `Ready`.
//! 2. **The in-flight step future is dropped.** A parked `run_async` future's
//!    own drop probe fires exactly once, when the run is dropped.
//! 3. **Synchronous scope cleanup still runs.** The context's step-returned
//!    overrides are cleared, which is observable as the fixture's own value
//!    becoming visible again.
//!
//! # Why the first assertion cannot stand alone
//!
//! "No `ScenarioOutcome` was observed" is true of *any* future dropped before
//! `Ready`, in every implementation, correct or broken. It therefore carries no
//! discriminating power at all, and the plan's INV-10 row records it as a
//! mandatory hardening requirement that the gate carry a progress witness — a
//! counter incremented inside the gate's own `poll`, asserted non-zero *before*
//! the drop assertions. Without it a test that never reached the gate would
//! pass for the same reason a correct one does.
//!
//! # Why there is no runtime here
//!
//! [`Waker::noop`](std::task::Waker::noop) polls a future directly, with no
//! executor. That is legal; what panics is a *Tokio* future being polled
//! outside a runtime, and this file never constructs one — the parked future is
//! hand-written, and the `Both`-mode steps run synchronously inside
//! `execute_step_async` because that is what `Both` means.
//!
//! # Why this is an integration binary (D21)
//!
//! The step case needs a genuinely registered [`StepExecutionMode::Async`] step,
//! and reaching the registry at all aborts the unit-test binary: `STEP_MAP` is a
//! `LazyLock` that eagerly asserts no two registrations collide, and
//! `registry/introspection.rs` registers one pattern twice *on purpose* to test
//! that assertion. So the plan's named artefact for INV-10 moves here from
//! `runner/tests/cancel.rs`, and D25 records why: that path was named before it
//! was known that the step case could be discharged directly at all, and D21's
//! rule — runner tests that resolve steps live in `tests/` — settles where.
//!
//! # The three hardening requirements, and where each is discharged
//!
//! 1. The progress witness is [`GATE_POLLS`], asserted non-zero by
//!    [`poll_until_gate_entered`] before any drop assertion runs.
//! 2. That poll loop is bounded and continues until the gate reports entered,
//!    rather than polling exactly once — `Waker::noop`'s `RawWaker` ignores
//!    `wake`, so a future re-polled only on a wake would hang forever.
//! 3. The drop probe is owned by the gate future itself, not by the closure
//!    that builds it, and [`GATE_DROPS`] is asserted **zero** before the drop
//!    so a probe that was never installed cannot satisfy the assertion after.
//!
//! # Why the counters are thread-local
//!
//! The cases in this binary may run concurrently: `cargo test` gives each test
//! its own thread, and nextest gives each its own process. A pair of `static`
//! counters would be shared by the first and private to the second, so the same
//! file would be correct under one runner and racy under the other. A
//! thread-local is private under both, because every poll in these cases happens
//! on the thread that made the assertion — there is no runtime to move the work
//! elsewhere. `#[serial]` would also work and is rejected: it would serialize
//! tests that have no shared state at all to hide a sharing that should not
//! exist.

use std::{
    any::Any,
    cell::Cell,
    future::Future,
    pin::Pin,
    task::{Context, Poll},
};

use rstest_bdd::{
    StepContext,
    StepError,
    StepExecution,
    StepExecutionMode,
    StepFuture,
    StepKeyword,
    StepPattern,
    runner::ScenarioPlanBuilder,
    submit,
};

#[path = "runner_cancel/cases.rs"]
mod cases;

thread_local! {
    /// How many times the parked gate's future has been polled.
    ///
    /// The progress witness of hardening requirement 1. Read by
    /// [`poll_until_gate_entered`] and asserted by the cancellation case.
    static GATE_POLLS: Cell<usize> = const { Cell::new(0) };

    /// How many times a gate's drop probe has fired.
    ///
    /// Asserted **zero** before the run is dropped and exactly one after, so
    /// neither a probe that was never installed nor a probe that fired early
    /// can satisfy the pair.
    static GATE_DROPS: Cell<usize> = const { Cell::new(0) };

    /// What the gate read out of the context's override map when it was entered.
    ///
    /// The cleanup case's *non-vacuity* witness. Reading the fixture cell after
    /// the run is not enough: `insert_value` writes into the context's override
    /// map rather than into the fixture's own cell, so the fixture reads the
    /// same value before and after cleanup and an assertion drawn from it would
    /// hold under a guard that never cleared anything. This records what was
    /// actually overridden, which is what makes "and it is gone afterwards" a
    /// statement about cleanup rather than about a cell nothing wrote to.
    static GATE_SAW: Cell<Option<u8>> = const { Cell::new(None) };
}

/// The most times the harness polls before declaring the gate unreachable.
///
/// A bound rather than a `loop`: a gate that is never entered must fail loudly
/// rather than hang, and a harness that hangs is worse than one that fails
/// because nextest's `terminate-after` would report it as an infrastructure
/// problem rather than a regression. The bound is generous — the run reaches
/// the gate on its first poll — so exceeding it means the gate is genuinely
/// unreachable rather than that the budget was tight.
pub(crate) const MAX_POLLS: usize = 64;

/// The fixture name the cleanup case's returned marker resolves to.
pub(crate) const MARKER: &str = "runner_cancel_marker";

/// A value a `Both`-mode step returns, and a fixture holds.
///
/// Distinct from every other type in this binary, because `insert_value`
/// matches on `TypeId` and an ambiguity would be reported as
/// [`ValueFate::AmbiguousIgnored`](rstest_bdd::runner::ValueFate) rather than as
/// the override this case needs.
#[derive(Debug, PartialEq, Eq)]
pub(crate) struct Marker(pub(crate) u8);

/// Fires [`GATE_DROPS`] when dropped.
///
/// Hardening requirement 3 lives here: this is a field of [`Gate`], so it is
/// owned by the future. A probe captured by the closure that *builds* the gate
/// would be dropped when that closure was, and the post-drop assertion would
/// then pass whether or not the gate had ever been polled.
struct DropProbe;

impl Drop for DropProbe {
    fn drop(&mut self) { GATE_DROPS.with(|drops| drops.set(drops.get() + 1)); }
}

/// A future that reports its first poll and then parks forever.
struct Gate {
    /// Exists to be dropped with this future, and is never read.
    _probe: DropProbe,
}

impl Future for Gate {
    type Output = Result<StepExecution, StepError>;

    fn poll(self: Pin<&mut Self>, _cx: &mut Context<'_>) -> Poll<Self::Output> {
        let _ = &self._probe;
        GATE_POLLS.with(|polls| polls.set(polls.get() + 1));
        Poll::Pending
    }
}

/// An `Async`-mode step whose handler parks forever.
///
/// Registered under [`StepExecutionMode::Async`], so `execute_step_async` takes
/// the `run_async` arm and awaits this future for real — which is what makes the
/// drop assertion evidence about the *driver's* await point rather than about a
/// test double placed beside it.
///
/// The override is read **here**, at construction, rather than inside `poll`:
/// this is the moment the driver is about to await a step handler, and the
/// value visible now is precisely what the run would have left behind had it
/// completed. Recording it is the non-vacuity witness for the cleanup assertion
/// in [`cases`](super::cases) — see this file's module documentation.
fn parks<'ctx>(
    ctx: &'ctx mut StepContext<'_>,
    _text: &'ctx str,
    _docstring: Option<&'ctx str>,
    _table: Option<&'ctx [&'ctx [&'ctx str]]>,
) -> StepFuture<'ctx> {
    GATE_SAW.with(|saw| saw.set(ctx.try_borrow::<Marker>(MARKER).ok().map(|marker| marker.0)));
    Box::pin(Gate {
        _probe: DropProbe,
    })
}

/// The sync arm of [`parks`], which must never be called.
///
/// Registering an `Async`-mode step still requires a `run` pointer, and this one
/// returns an error rather than a value: if the driver ever called it, the run
/// would stop at this step instead of parking, the gate would never be entered,
/// and [`poll_until_gate_entered`] would fail with "the gate was never entered",
/// which names the real fault better than a silent pass would.
fn sync_arm_was_called(
    _ctx: &mut StepContext<'_>,
    _text: &str,
    _docstring: Option<&str>,
    _table: Option<&[&[&str]]>,
) -> Result<StepExecution, StepError> {
    Err(StepError::ExecutionError {
        pattern: PARKING_STEP.to_owned(),
        function: "sync_arm_was_called".to_owned(),
        message: "an Async-mode step's sync arm must not be reached by the async driver".to_owned(),
    })
}

/// The pattern [`parks`] is registered under.
pub(crate) const PARKING_STEP: &str = "a runner cancellation gate parks";
/// The pattern the marker-returning step is registered under.
const RETURNING_STEP: &str = "a runner cancellation step returns a marker";
/// The pattern the ordinary `Both`-mode step is registered under.
const PASSING_STEP: &str = "a runner cancellation step passes";

/// The `StepPattern` objects the registrations match against.
static PARKING_PATTERN: StepPattern = StepPattern::new("a runner cancellation gate parks");
static RETURNING_PATTERN: StepPattern =
    StepPattern::new("a runner cancellation step returns a marker");
static PASSING_PATTERN: StepPattern = StepPattern::new("a runner cancellation step passes");

/// Return the marker a caller's fixture holds.
fn returns_marker(
    _ctx: &mut StepContext<'_>,
    _text: &str,
    _docstring: Option<&str>,
    _table: Option<&[&[&str]]>,
) -> Result<StepExecution, StepError> {
    let value: Box<dyn Any> = Box::new(Marker(7));
    Ok(StepExecution::from_value(Some(value)))
}

/// The async arm of [`returns_marker`]; the step is `Both`, so both arms exist.
fn returns_marker_async<'ctx>(
    ctx: &'ctx mut StepContext<'_>,
    text: &'ctx str,
    docstring: Option<&'ctx str>,
    table: Option<&'ctx [&'ctx [&'ctx str]]>,
) -> StepFuture<'ctx> {
    Box::pin(std::future::ready(returns_marker(
        ctx, text, docstring, table,
    )))
}

/// A step that resolves and does nothing.
fn passes(
    _ctx: &mut StepContext<'_>,
    _text: &str,
    _docstring: Option<&str>,
    _table: Option<&[&[&str]]>,
) -> Result<StepExecution, StepError> {
    Ok(StepExecution::from_value(None))
}

/// The async arm of [`passes`].
fn passes_async<'ctx>(
    ctx: &'ctx mut StepContext<'_>,
    text: &'ctx str,
    docstring: Option<&'ctx str>,
    table: Option<&'ctx [&'ctx [&'ctx str]]>,
) -> StepFuture<'ctx> {
    Box::pin(std::future::ready(passes(ctx, text, docstring, table)))
}

const _: () = {
    submit! {
        rstest_bdd::Step {
            keyword: StepKeyword::Given,
            pattern: &PARKING_PATTERN,
            run: sync_arm_was_called,
            run_async: parks,
            execution_mode: StepExecutionMode::Async,
            fixtures: &[],
            file: file!(),
            line: line!(),
        }
    }
    submit! {
        rstest_bdd::Step {
            keyword: StepKeyword::When,
            pattern: &RETURNING_PATTERN,
            run: returns_marker,
            run_async: returns_marker_async,
            execution_mode: StepExecutionMode::Both,
            fixtures: &[],
            file: file!(),
            line: line!(),
        }
    }
    submit! {
        rstest_bdd::Step {
            keyword: StepKeyword::Then,
            pattern: &PASSING_PATTERN,
            run: passes,
            run_async: passes_async,
            execution_mode: StepExecutionMode::Both,
            fixtures: &[],
            file: file!(),
            line: line!(),
        }
    }
};

/// The plan a cancellation case drives.
///
/// Two shapes rather than one, because the cleanup clause needs a step-returned
/// override to exist at the moment of cancellation. A plan with only the parking
/// step would cancel with an empty override map, and "the map is still empty
/// afterwards" is true of a run that never cleaned up.
pub(crate) fn plan(with_returned_value: bool) -> rstest_bdd::runner::ScenarioPlan {
    let builder = ScenarioPlanBuilder::new("Cancellation", "notes/cancel.md").at_line(1);
    if with_returned_value {
        builder
            .step_at(StepKeyword::When, RETURNING_STEP, 2)
            .step_at(StepKeyword::Given, PARKING_STEP, 3)
            .build()
    } else {
        builder
            .step_at(StepKeyword::Given, PARKING_STEP, 2)
            .build()
    }
}

/// A plan whose single invocation is an ordinary `Both`-mode passing step.
///
/// The completion control's plan. It cannot be [`plan`] with a flag, because a
/// plan that parks forever cannot complete.
pub(crate) fn completing_plan() -> rstest_bdd::runner::ScenarioPlan {
    ScenarioPlanBuilder::new("Completion", "notes/cancel.md")
        .at_line(1)
        .step_at(StepKeyword::Then, PASSING_STEP, 2)
        .build()
}

/// How many times the gate has been polled on this thread.
pub(crate) fn gate_polls() -> usize { GATE_POLLS.with(Cell::get) }

/// How many times a gate's drop probe has fired on this thread.
pub(crate) fn gate_drops() -> usize { GATE_DROPS.with(Cell::get) }

/// What the gate read out of the context's override map when it was installed.
///
/// `None` when no gate ran, which is what the completion control asserts.
pub(crate) fn observed_before_cancel() -> Option<u8> { GATE_SAW.with(Cell::get) }

/// Poll `run` until the gate reports entered, bounded by [`MAX_POLLS`].
///
/// Hardening requirement 2. Polling exactly once would be enough *today*,
/// because `execute_step_async` reaches an `Async` handler's `run_async` on the
/// first poll when every earlier step resolves `Ready` immediately. One added
/// `yield_now()` for fairness would silently change that, and this loop is what
/// makes the harness keep working rather than report a false failure — or,
/// worse, a false pass from a gate that was never reached.
///
/// Returns the number of run-level polls it took, so a caller can assert the run
/// did not reach the gate more than once.
pub(crate) fn poll_until_gate_entered<T>(mut run: Pin<&mut T>, cx: &mut Context<'_>) -> usize
where
    T: Future,
{
    for polls in 1..=MAX_POLLS {
        assert!(
            run.as_mut().poll(cx).is_pending(),
            "the run reached `Ready` on poll {polls} instead of parking at the gate",
        );
        if gate_polls() > 0 {
            return polls;
        }
    }
    panic!(
        "the gate was never entered within {MAX_POLLS} polls, so nothing was in flight to \
         cancel and every assertion about the drop would be vacuous"
    );
}
