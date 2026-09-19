//! Registration and harness support for the D11 panic-boundary tests.
//!
//! Split from `runner_panics.rs` for two reasons. The unwrapped registration
//! below is the whole point of the file, and it needs the registry's raw types
//! — which read better gathered here than interleaved with assertions; and the
//! panic-hook guard is stateful in a way whose constraints deserve to be stated
//! where they are enforced.
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
//! The lock covers the run only, so assertions are unsynchronised — which is
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
    StepContext,
    StepError,
    StepExecution,
    StepExecutionMode,
    StepFuture,
    StepKeyword,
    StepPattern,
    submit,
};

/// Serialises the silenced windows of this binary's tests.
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

unwrapped_step!(
    StepKeyword::Given,
    "an unwrapped step panics",
    panicking
);

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
