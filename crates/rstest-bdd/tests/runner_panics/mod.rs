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
//! assertion. That confinement is still the right shape, but the reason has
//! changed, and an earlier revision of this file got the reason wrong in a way
//! that cost the suite its diagnoses.
//!
//! # Two designs that both fail, and why
//!
//! **The first** installed a silent hook and restored the previous one from its
//! guard's `Drop`. That is a hard failure whenever the body unwinds, because
//! `panic::set_hook` *panics* when called from a panicking thread
//! (`library/std/src/panicking.rs`: "cannot modify the panic hook from a
//! panicking thread"), and `panic::take_hook` does the same. A panic raised
//! while already unwinding cannot itself unwind, so `std` prints "thread caused
//! non-unwinding panic. aborting." and calls `process::abort()`: the process
//! died with `SIGABRT` and no message at all.
//!
//! **The second** kept the restore but confined the window to the run, and
//! serialized the windows behind a `Mutex` so that two overlapping tests could
//! not restore each other's hook. It is deadlock-free and abort-free, and it
//! still loses messages. The lock protects the *hook*, not the *reporting*: a
//! second test's assertion, running unsynchronized by design, panics while the
//! window is open and its message is swallowed by the silent hook. Worse, the
//! window can be widened for the rest of the process — `Restored::drop` skipped
//! its restore entirely when the thread was already unwinding.
//!
//! The claim that shipped with that second design was "assertions are
//! unsynchronized, which is correct because assertions are the part that
//! touches no global state". The premise is true and the conclusion does not
//! follow, which is the whole defect: an assertion touches no global state, but
//! it *panics*, and panicking consults the process-global hook. Serializing the
//! writers of a global while leaving every reader uncoordinated is not
//! synchronization.
//!
//! # The design here: one permanent hook, one flag per thread
//!
//! A single delegating hook is installed once per process, and it consults a
//! **thread-local** flag: it forwards to the hook it wrapped unless *the
//! panicking thread* has a window open. So a test's deliberate step panics are
//! silenced on that test's thread, and every other thread's panics — the
//! assertion failures the previous design ate — reach the previous hook and
//! print in full.
//!
//! Nothing is restored because nothing is replaced, so the whole `SIGABRT`
//! family is gone rather than guarded against: `set_hook`/`take_hook` are
//! reachable only from an install that runs once, and the flag is per-thread
//! and self-healing, so a widened window degrades to "this thread prints"
//! rather than to a permanently silent process. There is no lock, so tests no
//! longer need serializing, and their order no longer matters.
//!
//! Two properties of the hook body are load-bearing and easy to break:
//!
//! - the hook must **not** be guarded with `thread::panicking()`. That function is false while the
//!   hook runs — `std` increments its panic counter before calling the hook and decrements it after
//!   — so such a guard would silently disable silencing altogether;
//! - [`SILENT`] must stay a `const`-initialized, destructor-free `Cell<bool>`. A thread-local that
//!   registers a destructor can be observed as destroyed from the hook, and `LocalKey::with` panics
//!   in that case; a panic inside the hook is itself an abort. `Cell<bool>` needs no drop, so the
//!   access cannot fail, and the `const` block rules out an initializer running from inside the
//!   hook.

use std::{cell::Cell, panic, sync::Once};

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

/// Installs the delegating hook exactly once per process.
static INSTALL: Once = Once::new();

thread_local! {
    /// Whether *this* thread's panics are currently being silenced.
    ///
    /// `const`-initialized and destructor-free deliberately; see the module
    /// note. A plain static rather than a `RefCell`, because the hook only ever
    /// reads it and the guard only ever replaces one bit.
    static SILENT: Cell<bool> = const { Cell::new(false) };
}

/// Install the delegating hook, once per process, keeping the previous one.
fn install_hook() {
    INSTALL.call_once(|| {
        let previous = panic::take_hook();
        panic::set_hook(Box::new(move |info| {
            // Read only this thread's flag. A panic on any other thread — an
            // assertion in a sibling test, which is exactly what the previous
            // design swallowed — falls through to the hook that was here first.
            if !SILENT.with(Cell::get) {
                previous(info);
            }
        }));
    });
}

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

/// Close this thread's silencing window on the way out, however it exits.
///
/// Carries the *previous* flag value rather than clearing to `false`, so a
/// nested window cannot strand the outer one by closing early. Nesting does not
/// occur in this binary today — every [`silenced`] call is one frame deep — but
/// the invariant costs one `bool` and the alternative fails silently.
///
/// The binding at the call site must be a named `let`, never a bare `let _`:
/// `let _ = Unsilence(..)` drops the guard immediately and turns silencing into
/// a no-op that no test would notice.
struct Unsilence(bool);

impl Drop for Unsilence {
    fn drop(&mut self) { SILENT.with(|silent| silent.set(self.0)); }
}

/// Silence this thread's panics for the duration of `body`.
///
/// `body` is expected to *return* rather than unwind — it is a `catch_unwind`
/// in every use here — but it no longer *has* to: the window closes from `Drop`
/// on both paths, and closing it touches only a thread-local flag.
pub(super) fn silenced<T>(body: impl FnOnce() -> T) -> T {
    install_hook();
    let previous = SILENT.with(|silent| silent.replace(true));
    let _unsilence = Unsilence(previous);
    body()
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
