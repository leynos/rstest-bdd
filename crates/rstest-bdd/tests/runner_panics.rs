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
//! It is also not a claim about a *panicking destructor*. `catch_unwind` cannot
//! observe one: a destructor that panics while another panic unwinds aborts the
//! process before any handler runs. D11 covers that case too, but in
//! `runner/scope.rs`'s `CleanupGuard`, which is where value cleanup happens —
//! not here.
//!
//! # Why this is an integration test
//!
//! D21: the unit-test binary cannot reach the registry at all, so a runner test
//! that merely *executes* must be an integration test. See `runner_wire.rs`.
//!
//! # Running these tests
//!
//! The deliberate panics are silenced by a process-global panic hook, which is
//! why this binary's tests are serialized — see `runner_panics/mod.rs`. The
//! silencing is confined to a [`panics::silenced`] window around each run, and
//! never wraps an assertion; the module note explains why that confinement is
//! load-bearing rather than tidy. Run it with
//! `cargo nextest run -p rstest-bdd -E 'binary(runner_panics)'`.

use rstest::rstest;
use rstest_bdd::{
    ExecutionError,
    StepContext,
    StepError,
    StepKeyword,
    execution::{StepExecutionRequest, execute_step_async},
    runner::{
        FailureKind,
        ScenarioPlanBuilder,
        ScenarioScope,
        ScenarioStatus,
        SourceLocation,
        StepStatus,
        run_scenario,
    },
};
use rstest_bdd_macros::given;

#[path = "runner_panics/mod.rs"]
mod panics;

use panics::silenced;

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
    assert!(
        file.ends_with("runner_panics/mod.rs"),
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

/// What one async step produced, with the two layers kept apart.
///
/// The separation is the point rather than a wrapper added for convenience:
/// [`Escaped`](Self::Escaped) answers "did an unwind leave the driver?" and
/// [`Returned`](Self::Returned) is the step's own result. Collapsing them into
/// one `Result` would make *the driver unwound* and *the step failed* the same
/// value, which is exactly the distinction the tests below exist to pin.
enum AsyncRun {
    /// The driver unwound; the payload escaped past `execute_step_async`.
    Escaped(Box<dyn std::any::Any + Send>),
    /// The driver returned the step's own result, as its contract requires.
    Returned(Result<Option<Box<dyn std::any::Any>>, ExecutionError>),
}

/// Run one async step under `catch_unwind`, returning what actually happened.
///
/// The `silenced` window closes here rather than in the caller, so no
/// assertion is ever inside it.
fn run_async_catching(text: &'static str) -> AsyncRun {
    let escaped = silenced(|| {
        // `let ... else` rather than `.expect(...)`, following the convention
        // `runner_wire.rs` records: `allow-expect-in-tests` covers `#[test]`
        // functions and `#[cfg(test)]` items, and this is neither.
        let Ok(runtime) = tokio::runtime::Builder::new_current_thread().build() else {
            panic!("the test's own runtime setup is broken, not the runner under test");
        };
        let mut ctx = StepContext::default();
        let request = StepExecutionRequest {
            index: 0,
            keyword: StepKeyword::Given,
            text,
            docstring: None,
            table: None,
            feature_path: "notes/panics.md",
            scenario_name: "Unwrapped async",
        };
        std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
            runtime.block_on(execute_step_async(&request, &mut ctx))
        }))
    });

    match escaped {
        Ok(result) => AsyncRun::Returned(result),
        Err(payload) => AsyncRun::Escaped(payload),
    }
}

/// The `(pattern, message)` a panicking async step's error must carry.
///
/// The classification half of [`run_async_catching`], split from it because the
/// two are different jobs: that function owns the boundary and this one owns
/// what the boundary produced. Each `let ... else` below names one way the run
/// can be wrong, and the escape case keeps the payload in its message rather
/// than discarding it — a regression here escapes instead of misclassifying, so
/// the printed payload is the only diagnosis a reader gets.
fn async_panic_identity(text: &'static str) -> (String, String) {
    let result = match run_async_catching(text) {
        AsyncRun::Returned(result) => result,
        AsyncRun::Escaped(escaped) => panic!(
            "execute_step_async must return rather than unwind, including when the panic happens \
             while the future is built rather than while it is polled; it escaped with {escaped:?}"
        ),
    };

    let Err(error) = result else {
        panic!("a panicking async step must fail the run rather than pass it");
    };
    let ExecutionError::HandlerFailed { error, .. } = &error else {
        panic!("the failure must be a HandlerFailed; it was {error:?}");
    };
    let StepError::PanicError {
        pattern, message, ..
    } = error.as_ref()
    else {
        panic!("the wrapped error must be a PanicError");
    };
    (pattern.clone(), message.clone())
}

/// The two async boundaries, one case each.
///
/// The async half of the boundary catches a panic raised *after* an await.
///
/// This test drives `execute_step_async` rather than `run_scenario`: the async
/// scenario entry point is EP-M3's deliverable and does not exist yet, so the
/// layer that owns this boundary is where the obligation is discharged. D11's
/// rationale is about `run_scenario`'s contract, and this is the same boundary
/// one level down — the place a future async driver will reach through.
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
