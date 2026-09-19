//! Registration and harness support for the D11 panic-boundary tests.
//!
//! Split from `runner_panics.rs` for two reasons. The unwrapped registration
//! below is the whole point of the file, and it needs the registry's raw types
//! — which read better gathered here than interleaved with assertions; and the
//! panic-hook guard is stateful in a way whose constraints deserve to be stated
//! where they are enforced.
//!
//! The async boundary helpers ([`AsyncRun`] and [`run_async_catching`]) live
//! here for the same reason rather than the same topic. They are the other
//! half of "how a run is driven": the file below keeps the assertions, and
//! these own the `catch_unwind` and the [`silenced`] window around it. That is
//! also what keeps the outer file inside the repository's 400-line limit
//! without an allowlist entry.
//!
//! # Why the panic hook is silenced, and how narrowly
//!
//! Every deliberate panic in these tests is *expected*: the driver is supposed
//! to catch it and return an outcome. The default hook prints a backtrace for
//! each one regardless, so a passing run would emit a screenful of stack traces
//! and a reader would reasonably conclude the harness was broken.
//!
//! Silencing is confined to [`silenced`], which wraps only the *run* — never an
//! assertion. That confinement is the load-bearing part, and it is not
//! cosmetic. `panic::set_hook` **panics** when called from a panicking thread
//! (`library/std/src/panicking.rs`: "cannot modify the panic hook from a
//! panicking thread"), and `panic::take_hook` does the same. A panic raised
//! while already unwinding cannot itself unwind, so `std` prints "thread caused
//! non-unwinding panic. aborting." and calls `process::abort()`. A guard that
//! restored the hook from `Drop` while an assertion unwound therefore killed
//! the process with `SIGABRT` and no message whatsoever — which is exactly what
//! an earlier revision of this file did.
//!
//! Keeping the window around the run alone means the guard's `Drop` only ever
//! executes on the normal path, where `set_hook` is legal. The `Drop` still
//! checks `thread::panicking()` and skips the restore, because a future edit
//! could widen the window again and a silent `SIGABRT` is a hard failure to
//! trace back to its cause. In that case the failure is still reported —
//! libtest prints `FAILED` regardless — but without its message.
//!
//! # Why this needs a lock
//!
//! The hook is global, not per-thread, so two runs overlapping would interleave
//! and one would restore the other's hook. The queue is a `Mutex` held for the
//! whole silenced body rather than a `serial_test` `#[serial]` attribute:
//! `#[serial]` is a silent no-op the moment a test forgets it or the key
//! drifts, whereas holding a lock in the guard makes mutual exclusion a
//! property of the type that must be held to silence the hook.
//!
//! The lock covers the run only, so assertions are unsynchronized — which is
//! correct, because assertions are the part that touches no global state.
//!
//! Poisoning is recovered rather than propagated, on the same reasoning as
//! `runner_instrumentation/capture.rs`: a test that panics while holding the
//! lock has already reported its own failure, and replacing that report with a
//! second panic about the lock helps nobody.

use std::{
    panic::{self, PanicHookInfo},
    sync::{Mutex, MutexGuard, PoisonError},
};

use rstest_bdd::{
    ExecutionError,
    StepContext,
    StepError,
    StepExecution,
    StepExecutionMode,
    StepFuture,
    StepKeyword,
    StepPattern,
    execution::{StepExecutionRequest, execute_step_async},
    submit,
};

/// Serializes the silenced windows of this binary's tests.
static HOOK: Mutex<()> = Mutex::new(());

/// Register a step through the raw form, with no `catch_unwind` anywhere.
///
/// `step!` takes a sync handler, an async handler, a fixture list, and an
/// execution mode. All four are named explicitly rather than using the
/// four-argument form that generates an async arm, because what *makes* this
/// registration unwrapped is that neither handler contains a `catch_unwind`,
/// and writing both out means a reader does not have to expand a macro to
/// confirm that. The generated async arm would not have changed it.
///
/// The pattern is a module-level `static` because `Step` stores a
/// `&'static StepPattern`, and a `static` cannot live inside a function body.
macro_rules! unwrapped_step {
    ($keyword:expr, $pattern:expr, $handler:path) => {
        const _: () = {
            fn __unwrapped_async<'ctx>(
                ctx: &'ctx mut StepContext<'_>,
                text: &'ctx str,
                docstring: Option<&'ctx str>,
                table: Option<&'ctx [&'ctx [&'ctx str]]>,
            ) -> StepFuture<'ctx> {
                Box::pin(std::future::ready($handler(ctx, text, docstring, table)))
            }

            static PATTERN: StepPattern = StepPattern::new($pattern);
            submit! {
                rstest_bdd::Step {
                    keyword: $keyword,
                    pattern: &PATTERN,
                    run: $handler,
                    run_async: __unwrapped_async,
                    execution_mode: StepExecutionMode::Both,
                    fixtures: &[],
                    file: file!(),
                    line: line!(),
                }
            }
        };
    };
}

/// The handler the raw registration points at, and the step it registers.
///
/// This is the case Constraint 3 names: a step registered through a form whose
/// wrapper the plan did not generate. The `panic!` below has nothing between it
/// and the driver, so it is the driver's own boundary or nothing.
fn panicking(
    _ctx: &mut StepContext<'_>,
    _text: &str,
    _docstring: Option<&str>,
    _table: Option<&[&[&str]]>,
) -> Result<StepExecution, StepError> {
    panic!("deliberate panic from an unwrapped step! handler");
}

unwrapped_step!(StepKeyword::Given, "an unwrapped step panics", panicking);

/// An `Async`-mode step whose async body panics *after* an await point.
///
/// Registered separately from [`panicking`] because it exercises a different
/// boundary. A synchronous handler panics while the call is on the stack, so a
/// single `catch_unwind` around the call observes it. An `async` body panics
/// while the *future is being polled*, which is a different frame and a
/// different boundary — `catch_unwind` around a synchronous call cannot see it,
/// which is why the async path uses `catch_unwind_future`. The `yield_now`
/// before the panic is load-bearing: it guarantees the handler actually
/// suspends, so the panic occurs on a resumed poll rather than on the first one,
/// which is the case a naive implementation would miss.
///
/// The body is a real `async fn` block inside the registration rather than a
/// named `async fn`, because `Step::run_async` takes a plain function pointer
/// returning a boxed future, so a named one would have to be written with
/// explicit lifetimes anyway.
fn panicking_async<'ctx>(
    _ctx: &'ctx mut StepContext<'_>,
    _text: &'ctx str,
    _docstring: Option<&'ctx str>,
    _table: Option<&'ctx [&'ctx [&'ctx str]]>,
) -> StepFuture<'ctx> {
    Box::pin(async {
        tokio::task::yield_now().await;
        panic!("deliberate panic from an unwrapped async step! handler");
    })
}

/// Register the async step, reusing the macro above for its registration shape
/// and overriding `run` with the async pointer.
///
/// Written out rather than added as a macro arm because the two registrations
/// differ in exactly the field under test — which handler is used and what mode
/// the step declares — and hiding that behind a macro parameter would make the
/// difference harder to see, not easier.
const _: () = {
    static PATTERN: StepPattern = StepPattern::new("an unwrapped async step panics");
    submit! {
        rstest_bdd::Step {
            keyword: StepKeyword::Given,
            pattern: &PATTERN,
            run: panicking,
            run_async: panicking_async,
            execution_mode: StepExecutionMode::Async,
            fixtures: &[],
            file: file!(),
            line: line!(),
        }
    }
};

/// An `Async`-mode step that panics while its future is being *built*.
///
/// Distinct from [`panicking_async`], which panics during a poll. The call that
/// constructs the future is itself user code, and `step!`'s four-argument form
/// with an explicit async mode generates exactly a constructor that evaluates
/// the synchronous handler eagerly:
///
/// ```text
/// fn __rstest_bdd_auto_async(..) -> StepFuture<'ctx> {
///     Box::pin(::std::future::ready($handler(..)))
/// }
/// ```
///
/// So the panic happens before a poll exists. A boundary that wrapped only the
/// poll would let this unwind out of `execute_step_async`, and — unlike the
/// synchronous case — there would be no `catch_unwind` anywhere between it and
/// the caller's test harness. The body panics unconditionally and *before* it
/// returns a future, so it is this case and not the polled one.
fn panicking_while_building<'ctx>(
    _ctx: &'ctx mut StepContext<'_>,
    _text: &'ctx str,
    _docstring: Option<&'ctx str>,
    _table: Option<&'ctx [&'ctx [&'ctx str]]>,
) -> StepFuture<'ctx> {
    panic!("deliberate panic while building an unwrapped async step future");
}

const _: () = {
    static PATTERN: StepPattern = StepPattern::new("an unwrapped step panics while building");
    submit! {
        rstest_bdd::Step {
            keyword: StepKeyword::Given,
            pattern: &PATTERN,
            run: panicking,
            run_async: panicking_while_building,
            execution_mode: StepExecutionMode::Async,
            fixtures: &[],
            file: file!(),
            line: line!(),
        }
    }
};

/// The shape `std::panic::take_hook` returns.
///
/// Named rather than spelled out at each use because the trait-object bounds
/// (`Sync + Send + 'static`) are three of the four tokens in it, and repeating
/// them obscures that this is simply "a panic hook".
type Hook = Box<dyn Fn(&PanicHookInfo<'_>) + Sync + Send + 'static>;

/// Restore a hook once the guarded window has closed.
///
/// Owns the lock as well as the hook, so the two cannot be released out of
/// order: a waiting test must not be able to install its hook while this one is
/// still un-restored.
struct Restored {
    /// Held for the window's duration; released after the hook is restored.
    _lock: MutexGuard<'static, ()>,
    /// The hook to put back, captured before the silent one was installed.
    previous: Option<Hook>,
}

impl Drop for Restored {
    fn drop(&mut self) {
        // The window is meant to close on the normal path, where `set_hook` is
        // legal. This check exists so that widening the window fails safe
        // rather than aborting the process; see the module note.
        if std::thread::panicking() {
            return;
        }
        if let Some(previous) = self.previous.take() {
            panic::set_hook(previous);
        }
    }
}

/// Run `body` with the panic hook silenced, and restore it afterwards.
///
/// `body` is expected to *return* rather than unwind — it is a `catch_unwind`
/// in every use here — because that is what keeps the restore legal. See the
/// module note for why that matters.
pub(super) fn silenced<T>(body: impl FnOnce() -> T) -> T {
    let guard = Restored {
        _lock: HOOK.lock().unwrap_or_else(PoisonError::into_inner),
        previous: Some(panic::take_hook()),
    };
    // The silent hook is installed after the previous one is captured, so
    // `previous` can never be the silent hook itself.
    panic::set_hook(Box::new(|_| {}));
    let result = body();
    drop(guard);
    result
}

/// What one async step produced, with the two layers kept apart.
///
/// The separation is the point rather than a wrapper added for convenience:
/// [`Escaped`](Self::Escaped) answers "did an unwind leave the driver?" and
/// [`Returned`](Self::Returned) is the step's own result. Collapsing them into
/// one `Result` would make *the driver unwound* and *the step failed* the same
/// value, which is exactly the distinction the tests that consume this exist to
/// pin.
///
/// It lives here rather than beside those tests because it is internal to the
/// boundary below: nothing outside this module constructs one.
enum AsyncRun {
    /// The driver unwound; the payload escaped past `execute_step_async`.
    Escaped(Box<dyn std::any::Any + Send>),
    /// The driver returned the step's own result, as its contract requires.
    Returned(Result<Option<Box<dyn std::any::Any>>, ExecutionError>),
}

/// Run one async step under `catch_unwind`, returning what actually happened.
///
/// The [`silenced`] window closes here rather than in the caller, so no
/// assertion is ever inside it — the confinement the module note requires.
///
/// Its only caller is [`async_panic_identity`], which is why it is private:
/// the enum above is an implementation detail of these two together, not a
/// shape the assertion file should be able to see.
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
///
/// It stays in this module while its only caller does not, because
/// [`AsyncRun`] is private to the boundary and the classification consumes it
/// directly. Widening the enum to `pub(super)` so the caller could match it
/// would expose the two-layer distinction to a file that has no use for it.
pub(super) fn async_panic_identity(text: &'static str) -> (String, String) {
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
