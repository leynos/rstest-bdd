//! Unit tests for the panic boundary in `execution::unwind`.
//!
//! These tests take the step's identity as given, so they need no registry at
//! all: [`guarded`] is the *only* function handed to them, and it asserts its
//! own result rather than looking anything up. That matters because D21 closes
//! the unit-test binary's registry — `registry/introspection.rs` registers a
//! duplicate pattern under `#[cfg(test)]`, so the first lookup of any kind
//! trips `STEP_MAP`'s own `assert!` and aborts.
//!
//! So this file is deliberately not a second copy of `tests/runner_panics.rs`.
//! That file owns the claim a caller can check — a panic reaches a *frontend*
//! as a returned outcome — and it pays for the claim by being an integration
//! test. What is left here is the payload mapping stated as a total function,
//! which is exactly the part a `catch_unwind` in a driver would have had to
//! duplicate.
//!
//! # What these tests do not claim
//!
//! `panic_message`'s *rendering* is not pinned here, only its input. A string
//! payload passes through unchanged, so requiring the raised text to come back
//! is a statement about the boundary rather than about the renderer; the
//! renderer's own downcast chain, including its formatting of non-string
//! payloads, is `panic_support`'s contract and is tested there. Asserting the
//! same rendering at two levels would mean two places to update when it
//! changes, and the lower one is already the authority. So the unrenderable
//! cases require only that *something* was produced, which is enough to catch a
//! boundary that dropped the message rather than delegating.
//!
//! The async boundary is asserted here only for the part that needs no step:
//! [`guarded_async`] hands the closure it is given to the handler slot, so the
//! panic it guards against can be raised by a closure rather than by a
//! registered handler. The end-to-end claim — that a *registered* async step's
//! panic reaches a frontend as a returned outcome — needs a runtime and a
//! registry entry, so it lives in `tests/runner_panics.rs` beside its
//! synchronous sibling; see INV-17.

use std::{
    any::Any,
    panic::{AssertUnwindSafe, catch_unwind},
};

use rstest::rstest;

use super::super::{guarded, unwind::guarded_async};
use crate::{
    Step,
    StepError,
    StepExecution,
    StepExecutionMode,
    StepFuture,
    StepKeyword,
    StepPattern,
    context::StepContext,
    skip::SkipRequest,
};

/// The async half of a raw registration whose synchronous half is [`unreachable`].
///
/// Written as a named function rather than `sync_to_async(unreachable)`, which
/// returns an opaque `impl FnOnce` and so cannot satisfy `Step::run_async`'s
/// function-pointer type. The body never runs — `guarded` calls the closure it
/// is handed — so it panics for the same reason its sibling does.
fn unreachable_async<'ctx>(
    ctx: &'ctx mut StepContext<'_>,
    text: &'ctx str,
    docstring: Option<&'ctx str>,
    table: Option<&'ctx [&'ctx [&'ctx str]]>,
) -> crate::StepFuture<'ctx> {
    Box::pin(std::future::ready(unreachable(ctx, text, docstring, table)))
}

/// A sync handler that must never be called through the step's own pointer.
fn unreachable(
    _ctx: &mut StepContext<'_>,
    _text: &str,
    _docstring: Option<&str>,
    _table: Option<&[&[&str]]>,
) -> Result<StepExecution, StepError> {
    panic!("guarded must call the closure it was given, not the step's own handler");
}

/// A step with the given pattern and identity, and no handler of its own.
///
/// `guarded` never calls `run`; it is given the closure it should call. So the
/// two function pointers only have to exist, and both panic if anything ever
/// reaches them — which is the control on the claim above: if a future refactor
/// made `guarded` invoke the step's own pointer, this file would fail loudly
/// rather than pass silently.
fn harness_step(pattern: &'static StepPattern, file: &'static str, line: u32) -> Step {
    Step {
        keyword: StepKeyword::Given,
        pattern,
        run: unreachable,
        run_async: unreachable_async,
        execution_mode: StepExecutionMode::Both,
        fixtures: &[],
        file,
        line,
    }
}

/// Build a harness step in one line, with a block-scoped pattern static.
///
/// `Step` stores a `&'static StepPattern`, so the pattern cannot be a temporary
/// and cannot be a local either — a `static` is the only thing that qualifies,
/// and one per call site is what keeps each test's pattern text beside the
/// assertion that reads it. The name is reused across expansions because each
/// lives in its own block, which is also what stops one test's pattern from
/// being visible to another's.
macro_rules! harness {
    ($pattern:literal, $file:literal, $line:literal) => {{
        static PATTERN: StepPattern = StepPattern::new($pattern);
        harness_step(&PATTERN, $file, $line)
    }};
}

/// How many polls a future is given before it is declared stuck.
///
/// `Waker::noop`'s `RawWaker` ignores `wake`, so a suspended future is only ever
/// re-polled by the loop driving it. Every future asserted here resolves on its
/// first poll, so this is a tripwire rather than a budget.
const MAX_POLLS: usize = 64;

/// The panic's rendered message, or `None` when the result was not a panic.
fn panic_message_of(result: Result<StepExecution, StepError>) -> Option<String> {
    match result {
        Err(StepError::PanicError { message, .. }) => Some(message),
        _ => None,
    }
}

/// A handler that returns an ordinary outcome is passed through untouched.
///
/// The control for the whole file: it proves the `catch_unwind` wrapping costs
/// nothing observable on the path every step actually takes. Without it, a
/// `guarded` that returned a `PanicError` unconditionally would satisfy the
/// panic tests below.
#[test]
fn an_ordinary_result_is_passed_through() {
    let step = harness!("a step", "notes/unwind.rs", 40);

    let returned = StepExecution::from_value(None);
    let result = guarded(&step, || Ok(returned));

    let Ok(StepExecution::Continue { .. }) = result else {
        panic!("a handler's own result must be returned unchanged; it was {result:?}");
    };
}

/// A handler's own `Err` is passed through, not reclassified as a panic.
///
/// The distinction matters downstream: `FailureKind::of` maps this error to
/// `Assertion` or `Other`, and mapping it to `Panic` would silently relabel
/// every failing step in every suite. The error is `ExecutionError`'s sibling
/// type rather than a value this file invents, so the assertion is about the
/// boundary's behaviour and not about `StepError`'s shape.
#[test]
fn a_handlers_own_error_is_not_reclassified() {
    let step = harness!("a failing step", "notes/unwind.rs", 55);

    let result = guarded(&step, || {
        Err(StepError::ExecutionError {
            pattern: "a failing step".to_owned(),
            function: "declared".to_owned(),
            message: "the handler returned this itself".to_owned(),
        })
    });

    let Err(StepError::ExecutionError { message, .. }) = result else {
        panic!("a handler's own error must survive unchanged; it was {result:?}");
    };
    assert_eq!(message, "the handler returned this itself");
}

/// A `SkipRequest` payload is read as a skip, and its message survives.
///
/// This is the arm that keeps `skip!` working through the boundary, and the
/// message is asserted rather than merely the variant: `into_message` consumes
/// the request, so a boundary that read the payload twice, or cloned the wrong
/// field, would still produce a `Skipped` while losing the reason. `skip!`
/// itself raises exactly this payload (`skip::SkipRequest::raise`), so the
/// witness is the real one rather than a reconstruction.
#[test]
fn a_skip_request_becomes_a_skip_with_its_message() {
    let step = harness!("a skippable step", "notes/unwind.rs", 78);

    let result = guarded(&step, || {
        SkipRequest::raise(Some("maintenance window".to_owned()))
    });

    let Ok(StepExecution::Skipped { message }) = result else {
        panic!("a SkipRequest payload is a skip, not a panic; it was {result:?}");
    };
    assert_eq!(
        message.as_deref(),
        Some("maintenance window"),
        "the skip's own reason must survive the boundary",
    );
}

/// A `SkipRequest` with no message stays a skip with no message.
///
/// Separate from the case above because `None` and `Some` take different paths
/// through `into_message` and through every frontend that renders a skip. A
/// boundary that unwrapped a missing message into an empty string would pass
/// the case above and fail this one.
#[test]
fn a_bare_skip_request_stays_message_less() {
    let step = harness!("a bare skip", "notes/unwind.rs", 95);

    let result = guarded(&step, || SkipRequest::raise(None));

    let Ok(StepExecution::Skipped { message }) = result else {
        panic!("a bare SkipRequest payload is a skip; it was {result:?}");
    };
    assert!(
        message.is_none(),
        "a skip raised without a reason must not acquire one; it was {message:?}",
    );
}

/// A non-`SkipRequest` payload becomes a `PanicError` carrying the step's own
/// registry identity and source.
///
/// The three fields are pinned together because each is independently
/// forgeable: a boundary could fill `pattern` from the wrong step, `function`
/// with a constant, and `message` from a downcast that fell through to
/// `Debug`. The step is constructed with a pattern, a file, and a line that
/// appear nowhere else in this file, so a copy from the environment is visible.
#[test]
fn a_foreign_payload_becomes_a_panic_error_identifying_the_step() {
    let step = harness!("a distinct pattern", "notes/unwind.rs", 118);

    // One call, destructured once. Calling `guarded` twice would let the two
    // assertions describe two different panics; the message is read out of the
    // same `PanicError` the identity fields come from.
    let Err(StepError::PanicError {
        pattern,
        function,
        message,
    }) = guarded(&step, || panic!("the handler's own message"))
    else {
        panic!("a foreign payload must produce a PanicError");
    };
    assert_eq!(
        message, "the handler's own message",
        "the panic's message must come from the raised payload",
    );
    assert_eq!(
        pattern, "a distinct pattern",
        "the pattern is the step's own registry spelling",
    );
    assert_eq!(
        function, "notes/unwind.rs:118",
        "the function field renders the step's file and line, since a raw registration records no \
         handler name",
    );
}

/// A panic raised by a *non-string* payload is still a `PanicError`.
///
/// `panic_message`'s downcast chain has a fallback for payloads that are
/// neither a `&str` nor a `String` — `panic::resume_unwind` from user code, or
/// one of `std`'s internal payload types. This pins that the boundary reaches
/// the fallback rather than propagating the payload: a `guarded` that
/// re-threw what it could not render would unwind out of `execute_step` for
/// exactly the payloads nobody tests by hand.
#[rstest]
#[case::integer_payload(Box::new(7_u32) as Box<dyn Any + Send>)]
#[case::unit_payload(Box::new(()) as Box<dyn Any + Send>)]
fn an_unrenderable_payload_is_still_a_panic(#[case] payload: Box<dyn Any + Send>) {
    let step = harness!("an odd panic", "notes/unwind.rs", 145);

    // `resume_unwind` rather than `panic!`: the payload is the argument, where
    // `panic!(non_string)` would format it and raise a `String` instead.
    let result = catch_unwind(AssertUnwindSafe(|| {
        guarded(&step, || std::panic::resume_unwind(payload))
    }));

    let Ok(result) = result else {
        panic!("the boundary must not re-throw a payload it cannot render");
    };
    assert!(
        panic_message_of(result).is_some_and(|message| !message.is_empty()),
        "an unrenderable payload must still render as *some* message",
    );
}

/// A panic while *building* an async step's future is caught too.
///
/// This is the negative control for the second boundary in [`guarded_async`],
/// and it is the shape `step!`'s four-argument form with `StepExecutionMode::
/// Async` actually registers: a constructor whose body is
/// `future::ready(handler(..))`, which evaluates the synchronous handler
/// eagerly. So the panic happens on the call, before any future exists to poll
/// — and a `guarded_async` that wrapped only the poll would let it unwind past
/// the caller entirely.
///
/// # Why the closure cannot return a future
///
/// The `build` closure panics instead of returning one, and the return type
/// still has to name [`StepFuture`] so the closure satisfies the parameter. That
/// is deliberate: it is what makes this test fail against a poll-only
/// implementation, where the panic would instead escape through the `Err` of a
/// `catch_unwind` placed inside the function rather than around the call.
///
/// The future is polled directly rather than under a runtime, because
/// `guarded_async` resolves on its first poll — the construction panics before
/// anything can suspend. `Waker::noop` is enough for that; a future that
/// actually parked would never be re-polled, which is why the loop below is
/// bounded rather than open-ended.
#[test]
fn a_panic_while_building_an_async_future_is_caught() {
    let step = harness!("an eagerly panicking step", "notes/unwind.rs", 300);

    let mut run = Box::pin(guarded_async(&step, || -> StepFuture<'_> {
        panic!("the constructor panicked before any future existed")
    }));
    let waker = std::task::Waker::noop();
    let mut cx = std::task::Context::from_waker(waker);

    // Bounded for the same reason the runner's own poll loops are: `noop`'s
    // `RawWaker` ignores `wake`, so a future that suspended would be re-polled
    // only here and the loop would never end.
    let outcome = (0..MAX_POLLS).find_map(|_| match run.as_mut().poll(&mut cx) {
        std::task::Poll::Ready(value) => Some(value),
        std::task::Poll::Pending => None,
    });
    let Some(outcome) = outcome else {
        panic!(
            "the construction panics before it can suspend, so the guard must resolve on the \
             first poll rather than staying pending for {MAX_POLLS}"
        );
    };

    assert_eq!(
        panic_message_of(outcome).as_deref(),
        Some("the constructor panicked before any future existed"),
        "a panic while building the future must be classified as a PanicError, exactly as a \
         poll-time panic is",
    );
}
