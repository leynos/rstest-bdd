//! D11 — the runner is panic-safe at its own boundary.
//!
//! `run_scenario` is a public entry point with a documented contract: every
//! step outcome reaches the caller through the returned `ScenarioOutcome`, and
//! the function does not panic (Constraint 3, ADR-018-TR2). The macro-generated
//! step wrapper satisfies that for attribute-registered steps, because it
//! installs its own `catch_unwind`
//! (`crates/rstest-bdd-macros/src/codegen/wrapper/emit/assembly/mod.rs`). It is
//! *not* satisfied for steps registered through the raw `step!` form, which
//! stores a bare function pointer and has no wrapper at all — and that form is
//! ADR-018's own extension story. A driver that relies on the wrapper therefore
//! relies on something a legitimate registration path does not have.
//!
//! This file constructs exactly that case. The step is registered through the
//! raw form, its body panics, and the assertions require a returned outcome
//! classifying the failure as `FailureKind::Panic`.
//!
//! # What these tests do not claim
//!
//! The `catch_unwind` in `run_catching` is the observable substitute for "a
//! panic reached the caller": it captures what a real caller would see, and the
//! assertion then requires that it captured nothing. It is not a claim that the
//! *process* is unwind-safe — that is `std`'s business — and it is not a
//! licence to absorb an unwind the driver did not intend, which is why each
//! test also asserts the resulting status and classification rather than merely
//! "it did not unwind".
//!
//! It is also not a claim about the *abort* case. A destructor that panics while
//! another panic unwinds aborts the process before any handler runs, and no
//! amount of nesting can catch that. Nothing here asserts about it.
//!
//! The ordinary destructor case is covered at the end of this file. A
//! step-returned value is dropped by the runner on three paths — the value
//! itself when nothing matched, the value itself when the match was ambiguous,
//! and the override it displaced when it took a fixture's place. Each is dropped
//! under a guard rather than bare. The fourth is `runner/scope.rs`'s
//! `CleanupGuard`, which covers only the run-end cleanup. The tests at this
//! file's end drive the first three through `run_scenario`, because that is the
//! contract they have to satisfy: a destructor panic must reach the caller as a
//! returned outcome, not as an unwind.
//!
//! # Why this is an integration test
//!
//! D21: the unit-test binary cannot reach the registry at all, so a runner test
//! that merely *executes* must be an integration test. See `runner_wire.rs`.
//!
//! # Running these tests
//!
//! The deliberate panics are silenced by a process-global panic hook that
//! consults a **per-thread** flag, so this binary's tests are not serialized and
//! their order does not matter — see `runner_panics/mod.rs`. The silencing is
//! confined to a [`panics::silenced`] window around each run and never wraps an
//! assertion; the module note explains why that confinement is load-bearing
//! rather than tidy, and why the two earlier designs both failed. Run it with
//! `cargo nextest run -p rstest-bdd -E 'binary(runner_panics)'`.

use std::{any::Any, cell::Cell, panic::AssertUnwindSafe};

use rstest::rstest;
use rstest_bdd::{
    ExecutionError,
    InsertOutcome,
    StepContext,
    StepError,
    StepKeyword,
    runner::{
        FailureKind,
        ScenarioOutcome,
        ScenarioPlanBuilder,
        ScenarioScope,
        ScenarioStatus,
        SourceLocation,
        StepStatus,
        ValueFate,
        run_scenario,
    },
};
use rstest_bdd_macros::given;

#[path = "runner_panics/mod.rs"]
mod panics;

use panics::{async_panic_identity, silenced};

/// A step registered through the *attribute* macro, so it carries a wrapper.
///
/// This is the control for [`an_unwrapped_step_panic_is_returned_not_thrown`].
/// Same plan shape, same panic, but a registration path whose wrapper catches
/// it first. The two tests together separate the driver's own boundary from the
/// wrapper's: if the driver's `catch_unwind` were removed, only the raw test
/// would fail, and if the wrapper's were removed, only the wrapped one would.
#[given("a wrapped step panics")]
fn a_wrapped_step_panics() {
    panic!("deliberate panic from an attribute-registered step");
}

/// Run a one-step plan under `catch_unwind`, returning the outcome or the
/// payload that escaped.
///
/// Lifted into a helper so the three tests below differ only in what they
/// assert, not in how they build the run. `AssertUnwindSafe` is required
/// because the closure captures a `StepContext` full of `RefCell`s; it is sound
/// for the same reason the driver's own assertion is — a panic mid-run leaves
/// fewer values, never a half-visible one.
fn run_catching(
    text: &'static str,
    line: u32,
) -> Result<rstest_bdd::runner::ScenarioOutcome, Box<dyn std::any::Any + Send>> {
    let mut ctx = StepContext::default();
    let plan = ScenarioPlanBuilder::new("Unwrapped", "notes/panics.md")
        .step_at(StepKeyword::Given, text, line)
        .build();
    std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
        let scope = ScenarioScope::new(&mut ctx);
        run_scenario(&plan, scope)
    }))
}

/// The driver's own boundary turns a raw-`step!` unwind into a returned
/// failure.
///
/// Before D11 this test failed at the `let Ok(outcome)` below: the unwind
/// travelled straight out of `run_scenario`, through this helper's
/// `catch_unwind` on the *caller's* side, and into the `Err` arm. That is the
/// failure mode the assertion names, so a regression is diagnosable from the
/// output alone — which matters, because the same regression is easy to
/// misread: the run does not crash, it simply returns nothing, and every
/// assertion below would be skipped rather than failing.
#[test]
fn an_unwrapped_step_panic_is_returned_not_thrown() {
    let escaped = silenced(|| run_catching("an unwrapped step panics", 3));

    let Ok(outcome) = escaped else {
        panic!("run_scenario must return rather than unwind; it escaped with {escaped:?}");
    };

    assert_eq!(
        outcome.status(),
        ScenarioStatus::Failed,
        "a panicking step is a terminal failure, not a pass and not a skip",
    );
    let Some(first) = outcome.steps().first() else {
        panic!("the plan has one invocation, so a record must exist");
    };
    assert_eq!(first.status(), StepStatus::Failed);
    assert_eq!(
        first.failure_kind(),
        Some(FailureKind::Panic),
        "a panic must classify as Panic; reusing Assertion would be the canonical mislabel, since \
         the step is the caller's own code failing rather than an assertion about the system \
         under test",
    );
}

/// The candidate this test exists to reject: a panic treated as a skip.
///
/// `skip!()` raises a `SkipRequest` payload, so a driver that mapped *every*
/// caught payload to a skip — rather than only a `SkipRequest` — would still
/// satisfy "does not unwind" while turning a crashed step into a green suite.
/// That is the worst outcome this milestone could ship, so the discrimination is
/// asserted directly: the run must not skip, and `skip()` must be `None`.
#[test]
fn an_unwrapped_step_panic_is_not_mistaken_for_a_skip() {
    let Ok(outcome) = silenced(|| run_catching("an unwrapped step panics", 3)) else {
        panic!("run_scenario must return rather than unwind");
    };

    assert_ne!(
        outcome.status(),
        ScenarioStatus::Skipped,
        "a panic is not a skip; `skip!` raises a SkipRequest payload and this payload is a plain \
         &str",
    );
    assert!(
        outcome.skip().is_none(),
        "no skip record may exist for a panicking step",
    );
    assert!(
        outcome.failure().is_some(),
        "the panic must be recorded as the terminal failure",
    );
}

/// The panic classification is substantive, not merely present.
///
/// `FailureKind::Panic` is reachable only through `StepError::PanicError`, so
/// asserting it asserts that the driver built that variant with the registry's
/// own identity in it — the pattern the step was registered under, the function
/// the registry points at, and the panic's own message. Those three are what
/// `step-error-panic` renders, so an implementation filling any of them with a
/// placeholder would still classify as `Panic` while producing a useless
/// diagnostic. This test pins all three, and the plan-side path and line with
/// them.
#[test]
fn the_panic_carries_the_registry_identity_and_the_plans_source() {
    let Ok(outcome) = silenced(|| run_catching("an unwrapped step panics", 7)) else {
        panic!("run_scenario must return rather than unwind");
    };

    let Some(first) = outcome.steps().first() else {
        panic!("the plan has one invocation, so a record must exist");
    };

    assert_eq!(
        first.source().map(SourceLocation::path),
        Some("notes/panics.md"),
        "the plan's non-feature path survives the panic path",
    );
    assert_eq!(first.source().map(SourceLocation::line), Some(7));

    let Some(ExecutionError::HandlerFailed { error, text, .. }) = first.error() else {
        panic!("the failure must be a HandlerFailed carrying a StepError");
    };
    assert_eq!(
        text, "an unwrapped step panics",
        "the error carries the invocation's text",
    );
    let StepError::PanicError {
        pattern,
        function,
        message,
    } = error.as_ref()
    else {
        panic!("the wrapped error must be a PanicError");
    };
    assert_eq!(
        pattern, "an unwrapped step panics",
        "the pattern is the registry's own spelling, taken from the step rather than \
         reconstructed from the invocation",
    );
    // The driver cannot recover the handler's *name*: a `Step` records only
    // where it was defined, and the macro wrapper fills this field from
    // `stringify!` only because it *is* the generated code. So a raw
    // registration reports `file:line`, which is the crate's existing answer
    // (`MissingFixturesDetails::step_location`) and is actionable — it points
    // at the line a reader has to open. Both coordinates are asserted, because
    // a placeholder would satisfy neither.
    let (file, line) = function
        .rsplit_once(':')
        .expect("the function field must render as `file:line`");
    // `file!()` embeds the platform's own separator, so the comparison is made
    // against a normalized copy: on Windows the raw value ends in
    // `runner_panics\mod.rs` and a literal `/` comparison cannot hold. This is
    // the convention the crate's other tests already use
    // (`feature_rebuild_invalidation/harness/fixtures.rs`,
    // `trybuild_macros/staging.rs`). The raw value is kept for the failure
    // message so a reader sees what was actually produced rather than the
    // normalized form that was compared.
    let normalized = file.replace('\\', "/");
    assert!(
        normalized.ends_with("runner_panics/mod.rs"),
        "the file must be the module the unwrapped handler is defined in; it was `{file}`",
    );
    assert!(
        line.parse::<u32>().is_ok_and(|n| n > 0),
        "the line must be the handler's own declaration line, not a zero placeholder; it was \
         `{line}`",
    );
    assert!(
        message.contains("deliberate panic from an unwrapped step! handler"),
        "the panic's own message must survive into the error; it was `{message}`",
    );
}

/// An attribute-wrapped panic still behaves exactly as it did before D11.
///
/// The wrapper catches first, so the driver's boundary is never reached. This
/// is the non-regression half: D11's change must not alter the classification
/// or the message for the registration path that already worked, and the
/// assertion on the message proves the wrapper's rendering still wins rather
/// than being replaced by the driver's.
#[test]
fn a_wrapped_step_panic_is_unchanged() {
    let escaped = silenced(|| run_catching("a wrapped step panics", 4));

    let Ok(outcome) = escaped else {
        panic!("run_scenario must return rather than unwind; it escaped with {escaped:?}");
    };

    assert_eq!(outcome.status(), ScenarioStatus::Failed);
    let Some(first) = outcome.steps().first() else {
        panic!("the plan has one invocation, so a record must exist");
    };
    assert_eq!(first.failure_kind(), Some(FailureKind::Panic));
    let Some(ExecutionError::HandlerFailed { error, .. }) = first.error() else {
        panic!("the failure must be a HandlerFailed carrying a StepError");
    };
    let StepError::PanicError { message, .. } = error.as_ref() else {
        panic!("the wrapped error must be a PanicError");
    };
    assert!(
        message.contains("deliberate panic from an attribute-registered step"),
        "the wrapper's own message must survive; it was `{message}`",
    );
}

/// The async boundary, driven through its two cases.
///
/// The async half of the boundary catches a panic raised *after* an await.
///
/// This test drives `execute_step_async` directly rather than
/// `run_scenario_async`, and the scope of what it covers is worth stating
/// precisely: it discharges the *execution layer's* panic boundary, which is
/// the same boundary one level down from the one D11's rationale is about. It
/// does not exercise `run_scenario_async`'s own driving loop or its outcome
/// assembly, so a defect there would not show up here. Testing the layer that
/// owns the boundary keeps the failure attributable to the boundary itself.
///
/// The distinction it exists to catch is real rather than theoretical. A
/// synchronous `catch_unwind` around `(run_async)(..)` wraps only the *call*,
/// which merely constructs the future; an `async` body that panics after its
/// first suspension does so while the future is being polled, in a different
/// frame that the outer guard has already left. The `yield_now` in the
/// registered body forces exactly that. So this test fails against an
/// implementation that catches only the construction, and passes against one
/// that catches the poll — which is the whole reason the async path uses
/// `catch_unwind_future` instead of `guarded`.
///
/// A table rather than two functions because the two cases assert the *same*
/// relation over different inputs: the registered pattern survives as the
/// error's pattern, and the panic's own message survives the unwind. Two
/// functions would be two copies of that relation, free to drift apart.
///
/// What differs is which frame panics, and that difference is load-bearing
/// rather than incidental. `step!`'s four-argument form with an explicit async
/// mode registers a constructor whose body is `future::ready(handler(..))`, so
/// the synchronous handler runs *eagerly*, to build the future. A boundary
/// around the poll alone leaves that panic travelling out of
/// `execute_step_async` before a future exists to poll — which is why
/// `poll_time` is not merely a second sample of the first case. An
/// implementation that guarded only the poll fails the `build_time` case at the
/// `let Ok(result)` inside [`run_async_catching`], and the failure is the
/// escaping payload rather than a wrong classification; that helper's message
/// names both boundaries so the output is diagnosable either way.
#[rstest]
#[case::poll_time(
    "an unwrapped async step panics",
    "deliberate panic from an unwrapped async step! handler"
)]
#[case::build_time(
    "an unwrapped step panics while building",
    "deliberate panic while building an unwrapped async step future"
)]
fn an_unwrapped_async_step_panic_is_returned_not_thrown(
    #[case] text: &'static str,
    #[case] expected_message: &str,
) {
    let (pattern, message) = async_panic_identity(text);

    assert_eq!(pattern, text, "the pattern is the registry's own spelling");
    assert!(
        message.contains(expected_message),
        "the panic's message must survive the unwind; it was `{message}`",
    );
}

// --- destructor panics: the runner's drop paths -------------------------

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
