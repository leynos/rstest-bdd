//! The runner's drop paths, and the destructor panics they must survive.
//!
//! A step-returned value is dropped by the runner on three mid-run paths: the
//! value itself when nothing matched, the value itself when the match was
//! ambiguous, and the override it displaced when it took a fixture's place.
//! Each is dropped under a guard rather than bare, and each guard sits at its
//! own site — the first two inside `StepContext::insert_value`, the third in
//! `ValueFate`'s conversion one call away.
//!
//! These tests drive all three through `run_scenario`, because that is the
//! contract they have to satisfy: a destructor panic must reach the caller as a
//! returned outcome, not as an unwind. `runner/scope.rs`'s `CleanupGuard` is the
//! runner's fourth drop site and covers only the run-end cleanup, so it is not
//! exercised here.
//!
//! Split from the parent file to stay inside the repository's 400-line cap,
//! along the seam the parent's own section divider already marked.

use std::{any::Any, cell::Cell, panic::AssertUnwindSafe};

use rstest::rstest;
use rstest_bdd::{
    InsertOutcome,
    StepContext,
    StepKeyword,
    runner::{ScenarioOutcome, ScenarioPlanBuilder, ScenarioScope, ValueFate, run_scenario},
};
use rstest_bdd_macros::given;

use super::panics::silenced;

thread_local! {
    /// How many *armed* values this thread has dropped.
    ///
    /// Counts only the [`PanicOnDrop`] values whose destructor panics, and that
    /// is what makes it a usable signal rather than a tally of every drop in the
    /// process: a quiet value can be dropped on any frame, including a test's
    /// own, so counting those would make the delta depend on how the fixtures
    /// happen to be built. An armed value is created in exactly two places —
    /// inside the step handler, and on the runner's frame as the override it
    /// displaces — and in both cases only the **runner** can drop it. A nonzero
    /// delta therefore means the runner dropped an armed value, which is
    /// precisely the event under test.
    ///
    /// Per-thread rather than a process-wide `AtomicUsize`, because these tests
    /// are deliberately *not* serialized (see this file's module note) and share
    /// one binary: a global counter would have the three rows below counting each
    /// other's drops. Every drop here happens inline on the thread that ran the
    /// scenario, so a thread-local is exactly as accurate and needs no lock.
    ///
    /// `const`-initialized and destructor-free, for the reason the module note
    /// gives: a TLS value with a destructor is a hazard inside a panic hook.
    static ARMED_DROPS: Cell<usize> = const { Cell::new(0) };
}

/// A value that panics in its destructor, or one that does not.
///
/// The quieter variant is the one a **test** can own. Whether a given value
/// detonates is fixed when it is built and never changes, so a test may hold
/// quiet values for as long as it likes and confine every armed value to a
/// place only the runner will reach: the step's return, or the override that
/// return displaces.
///
/// `Drop` is infallible by signature, so a destructor that cannot complete has
/// nowhere to report that except by panicking. That is the shape the runner must
/// survive: the drop lands on the runner's own frame, inside `run_scenario`,
/// which owes its callers a returned outcome rather than an unwind.
struct PanicOnDrop {
    /// Whether the destructor detonates when this value is dropped.
    panics: bool,
}

impl Drop for PanicOnDrop {
    fn drop(&mut self) {
        if self.panics {
            ARMED_DROPS.with(|count| count.set(count.get() + 1));
            panic!("deliberate panic from a step-returned value's destructor");
        }
    }
}

/// A step returning a value that panics when dropped.
///
/// Registered through the attribute macro rather than the raw `step!` form, and
/// that choice keeps these tests about the *destructor*: the wrapper catches a
/// panic raised while the handler **runs**, so a step that comes back `Ok`
/// proves the destructor had not run yet. Any panic asserted about below can
/// therefore only have come from a drop.
///
/// This is the armed return, for the two rows where the runner drops the step's
/// own value. The displaced row uses [`a_step_returns_a_quiet_value`] instead,
/// because its armed value is the override the run *replaces* — a second
/// detonation on the return would leave the count unable to say which guard
/// fired.
#[given("a step returns a value that panics when dropped")]
fn a_step_returns_a_panic_on_drop_value() -> PanicOnDrop { PanicOnDrop { panics: true } }

/// A step returning a value whose destructor is well behaved.
///
/// Two registered steps rather than one with a mode, because the step registry
/// keys on text alone: the plan is where a row chooses which value it gets, and
/// a table row that needs a different return says so in its own line rather than
/// through a flag read somewhere else.
#[given("a step returns a quiet value")]
fn a_step_returns_a_quiet_value() -> PanicOnDrop { PanicOnDrop { panics: false } }

/// Which fixture arrangement the run is given before the returning step.
///
/// The three cases are the three ways the runner drops a step-returned value,
/// and they differ only in what is already registered — so they are a table
/// rather than three functions that differ in one line.
#[derive(Clone, Copy)]
enum Arrangement {
    /// No fixture of the value's type: the returned value matches nothing, and
    /// `insert_value` drops it under its own guard.
    Unmatched,
    /// One fixture of the type, so the step's return is `Inserted` — and an
    /// override is seeded into the context first, which the return then
    /// *displaces*. The displaced override is dropped by `ValueFate`'s
    /// conversion, on the runner's frame, which is the path a `_` binding in
    /// that conversion would have covered up. This row therefore pins the
    /// second guard rather than the first.
    ///
    /// The seeded override is the armed value and the return is quiet, which is
    /// what keeps the two guards separable: the *fixture* is only what
    /// `insert_value` matches on, so it merely has to exist, while the value
    /// reaching `ValueFate` through `InsertOutcome::Inserted` is whichever one
    /// was already in `values`.
    ///
    /// The cell-shaped alternative looks equivalent and is not.
    /// `insert_owned` stores a *fixture* as a borrowed cell, so `insert_value`
    /// would displace the empty `RefCell` wrapper rather than the armed value
    /// inside it — the row would then pass whether or not the conversion's guard
    /// existed. A fixture and an override live in different maps, and only the
    /// override map is what `insert_value` replaces into. This row is the
    /// regression test for exactly that mistake.
    Displaced,
    /// Two fixtures of the type: the returned value is dropped as ambiguous,
    /// under the second guard in `insert_value`.
    Ambiguous,
}

/// How many fixtures of the returned value's type `arrangement` registers.
const fn fixture_count(arrangement: Arrangement) -> usize {
    match arrangement {
        Arrangement::Unmatched => 0,
        Arrangement::Displaced => 1,
        Arrangement::Ambiguous => 2,
    }
}

/// The step text `arrangement` plans, naming the value that row wants returned.
const fn returning_step(arrangement: Arrangement) -> &'static str {
    match arrangement {
        Arrangement::Unmatched | Arrangement::Ambiguous => {
            "a step returns a value that panics when dropped"
        }
        Arrangement::Displaced => "a step returns a quiet value",
    }
}

/// Run the one-step plan, returning the outcome or the payload that escaped.
///
/// Fixture values are owned here and outlive the context that borrows them, and
/// every one of them is quiet. The armed value differs by row, and in all three
/// it is one no frame of the test can reach: the step's return in the two rows
/// where the runner drops it, and a seeded override in the row where the return
/// displaces what it finds.
fn run_drop_catching(arrangement: Arrangement) -> Result<ScenarioOutcome, Box<dyn Any + Send>> {
    let mut ctx = StepContext::default();
    let quiet: Vec<PanicOnDrop> = (0..fixture_count(arrangement))
        .map(|_| PanicOnDrop { panics: false })
        .collect();

    for (index, value) in quiet.iter().enumerate() {
        let name = if index == 0 { "first" } else { "second" };
        ctx.insert(name, value);
    }

    if matches!(arrangement, Arrangement::Displaced) {
        // Through `insert_value`, not `insert_owned`: only a value in the
        // context's override map is what the step's own return displaces, and
        // only such a value is dropped by `ValueFate`'s conversion. A failure
        // here means this scaffolding is broken rather than the runner, so the
        // message says which expectation the setup just failed.
        let seeded = ctx.insert_value(Box::new(PanicOnDrop { panics: true }));
        assert!(
            matches!(seeded, InsertOutcome::Inserted(None)),
            "the seeded override must be recorded, with nothing displaced yet; a fixture of its \
             type is registered above, so anything else means the scaffolding is wrong",
        );
    }

    let plan = ScenarioPlanBuilder::new("Drop", "notes/panics.md")
        .step_at(StepKeyword::Given, returning_step(arrangement), 3)
        .build();
    std::panic::catch_unwind(AssertUnwindSafe(|| {
        let scope = ScenarioScope::new(&mut ctx);
        run_scenario(&plan, scope)
    }))
}

/// The runner drops a step-returned value without unwinding.
///
/// One row per drop path. Each row asserts the same relation over a different
/// arrangement, and each pins a *different* guard, so a fix that covered only one
/// of the three would fail the others rather than passing on a shared code path:
///
/// - `unmatched` and `ambiguous` are both inside `StepContext::insert_value`;
/// - `displaced` is in the `ValueFate` conversion, one call away, and is the one a `_` binding
///   would have hidden.
///
/// The assertion is deliberately two-sided. The run must return an outcome at
/// all — an unwind here escapes `run_scenario` and reaches the caller's
/// `catch_unwind` instead — *and* the armed-drop count must have advanced. A
/// guard that retained the value rather than dropping it would satisfy "no
/// unwind" while leaking a step's returned value into the rest of the run.
///
/// The count is read before each run and compared as a delta, so the rows do not
/// have to agree about how many drops earlier rows caused, and the test does not
/// depend on execution order.
///
/// `Inserted` is asserted for the single-fixture row because that row is also
/// where the fate could be lost: a runner that dropped the *returning* value
/// instead of keeping it would report `NoMatch` and still pass a count-only
/// assertion.
#[rstest]
#[case::unmatched(Arrangement::Unmatched, ValueFate::NoMatch)]
#[case::displaced(Arrangement::Displaced, ValueFate::Inserted)]
#[case::ambiguous(Arrangement::Ambiguous, ValueFate::AmbiguousIgnored)]
fn a_panicking_destructor_does_not_escape_run_scenario(
    #[case] arrangement: Arrangement,
    #[case] expected_fate: ValueFate,
) {
    let before = ARMED_DROPS.with(Cell::get);
    let escaped = silenced(|| run_drop_catching(arrangement));

    let Ok(outcome) = escaped else {
        panic!(
            "a step-returned value's destructor panic must reach the caller as an outcome, not an \
             unwind; it escaped with {escaped:?}",
        );
    };
    assert_eq!(
        ARMED_DROPS.with(Cell::get) - before,
        1,
        "the runner must drop exactly the one armed value this run displaced or refused; a count \
         of 0 means it retained the value, and 2 means the drop happened twice",
    );
    let Some(first) = outcome.steps().first() else {
        panic!("the plan has one invocation, so a record must exist");
    };
    assert_eq!(
        first.value_insertion(),
        Some(expected_fate),
        "guarding the drop must not change what the run recorded about the value's fate",
    );
}
