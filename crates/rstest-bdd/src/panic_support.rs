//! Helpers for rendering panic payloads.

use std::{
    any::Any,
    future::Future,
    pin::Pin,
    task::{Context, Poll},
};

use crate::localization;

/// Extracts a panic payload into a human-readable message.
///
/// Attempts to downcast common primitives before falling back to an opaque
/// description that includes the payload [`std::any::TypeId`].
///
/// # Examples
/// ```
/// use rstest_bdd::panic_message;
///
/// let err = std::panic::catch_unwind(|| panic!("boom")).expect_err("expected panic");
/// assert_eq!(panic_message(err.as_ref()), "boom");
/// ```
#[must_use]
pub fn panic_message(e: &(dyn std::any::Any + Send)) -> String {
    macro_rules! try_downcast {
        ($($ty:ty),* $(,)?) => {
            $(
                if let Some(val) = e.downcast_ref::<$ty>() {
                    return val.to_string();
                }
            )*
        };
    }

    try_downcast!(
        &str,
        String,
        std::fmt::Arguments,
        Box<str>,
        bool,
        char,
        i8,
        u8,
        i16,
        u16,
        i32,
        u32,
        i64,
        u64,
        i128,
        u128,
        isize,
        usize,
        f32,
        f64,
    );
    // ``()`` lacks a ``Display`` implementation, so ``try_downcast!`` cannot
    // render it using ``to_string``.
    if e.downcast_ref::<()>().is_some() {
        return "()".to_owned();
    }

    let ty = format!(
        "erased `Any` payload ({:?}); panic with Display/Debug data for detail",
        e.type_id()
    );
    localization::message_with_args("panic-message-opaque-payload", |args| {
        args.set("type", ty);
    })
}

/// Drop a value, catching an unwind from its destructor and logging it.
///
/// The value is dropped on **every** path, which is what makes the return type
/// `Option<String>` rather than `Result<_, String>`: there is no branch a caller
/// should take differently, and the only question left is whether the drop went
/// quietly. A `Result` would invite an `?` at a call site where there is nothing
/// to recover from.
///
/// `pub(crate)` rather than public, because the runner's two drop sites are the
/// only callers and this crate's public surface is permanent. If a frontend ever
/// needs it, it can be promoted; the reverse is not true.
///
/// # Why a destructor needs a guard at all
///
/// A destructor that panics *while another panic unwinds* aborts the process
/// before any handler runs, which no amount of nesting can catch. This guard is
/// for the ordinary case: dropping a value the runner displaced mid-run, where
/// an unwind would otherwise travel straight out of `run_scenario` and break the
/// contract that every failure reaches the caller as a returned outcome.
///
/// # Panics
///
/// Does not panic. A panicking destructor is caught and reported through the
/// return value rather than propagated.
///
/// # Examples
///
/// ```ignore
/// // A value whose destructor is well behaved drops silently.
/// assert!(drop_guarded(Box::new(7_u32)).is_none());
///
/// // One whose destructor panics is reported instead of unwinding.
/// struct Detonates;
/// impl Drop for Detonates {
///     fn drop(&mut self) { panic!("destructor panic"); }
/// }
/// assert_eq!(drop_guarded(Detonates).as_deref(), Some("destructor panic"));
/// ```
///
/// The block is `ignore`d rather than run, and the reason is the function's own
/// visibility: a doctest is compiled as an external crate, so `use
/// rstest_bdd::panic_support::drop_guarded` is `E0603` for a `pub(crate)` item
/// and there is no crate-local form that runs. The behaviour is covered rather
/// than merely illustrated — `src/panic_support.rs`'s own `#[cfg(test)]` module
/// asserts both arms, and `tests/runner_panics.rs` drives all three drop sites
/// through `run_scenario`.
pub(crate) fn drop_guarded<T>(value: T) -> Option<String> {
    // `T` is not `UnwindSafe`: a `Box<dyn Any>` holding a step-returned value
    // carries no such guarantee, and the moved-into-a-`FnOnce` form does not
    // require one. Asserting it is sound in the sense that matters here — the
    // value is dropped exactly once, so no other reference can observe a
    // partial drop.
    match std::panic::catch_unwind(std::panic::AssertUnwindSafe(move || drop(value))) {
        Ok(()) => None,
        Err(payload) => Some(panic_message(payload.as_ref())),
    }
}

/// Log a destructor panic reported by [`drop_guarded`], and do nothing else.
///
/// A named function rather than an inline `if let` at each call site, so the
/// shape of the response exists once: the panic is reported and the operation
/// is **not** failed. There is no outcome a caller could take differently — the
/// value is gone either way, and the state around it is already consistent — so
/// four call sites would otherwise repeat the same three-line warning, free to
/// drift apart on its wording.
///
/// The message is the panic's own, not the dropped value's type: the type has
/// already been erased by the time this runs, and a `TypeId` would name an
/// erased `Any` rather than a fixture the reading developer could act on.
///
/// # Panics
///
/// Does not panic. A silent drop and a reported one take the same path with the
/// same calls.
pub(crate) fn report_drop_panic(reason: Option<String>) {
    if let Some(reason) = reason {
        tracing::warn!(
            %reason,
            "a step-returned value panicked while being dropped by the runner",
        );
    }
}

/// A future combinator that converts unwinds into `Err(payload)`.
///
/// This is used by macro-generated async step wrappers so they can intercept
/// `skip!` panics and convert them into [`crate::StepExecution::Skipped`]
/// outcomes without depending on external future utilities.
///
/// The implementation wraps polling in [`std::panic::AssertUnwindSafe`] so the
/// wrapper can catch panics from futures that are not [`std::panic::UnwindSafe`]
/// (for example, futures that hold mutable borrows to fixtures). If the wrapped
/// future panics, any captured state may have been left in an inconsistent
/// state; the wrapper exists to surface the panic payload, not to provide unwind
/// safety guarantees for that state.
pub struct CatchUnwindFuture<F>(Pin<Box<F>>);

impl<F> CatchUnwindFuture<F> {
    /// Wrap the provided future.
    ///
    /// # Examples
    ///
    /// ```
    /// use rstest_bdd::panic_support::CatchUnwindFuture;
    ///
    /// let mut future = Box::pin(CatchUnwindFuture::new(async { 42u8 }));
    /// let waker = std::task::Waker::noop();
    /// let mut cx = std::task::Context::from_waker(&waker);
    /// match future.as_mut().poll(&mut cx) {
    ///     std::task::Poll::Ready(Ok(value)) => assert_eq!(value, 42),
    ///     other => panic!("expected ready Ok(42), got {other:?}"),
    /// }
    /// ```
    pub fn new(inner: F) -> Self { Self(Box::pin(inner)) }
}

impl<F> Future for CatchUnwindFuture<F>
where
    F: Future,
{
    type Output = Result<F::Output, Box<dyn Any + Send>>;

    fn poll(self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<Self::Output> {
        let inner = &mut self.get_mut().0;

        match std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| inner.as_mut().poll(cx))) {
            Ok(Poll::Ready(output)) => Poll::Ready(Ok(output)),
            Ok(Poll::Pending) => Poll::Pending,
            Err(payload) => Poll::Ready(Err(payload)),
        }
    }
}

/// Wrap a future and return a [`CatchUnwindFuture`] that converts unwinds into `Err(payload)`.
///
/// # Examples
///
/// ```
/// use rstest_bdd::panic_support::catch_unwind_future;
///
/// let mut future = Box::pin(catch_unwind_future(async { panic!("boom") }));
/// let waker = std::task::Waker::noop();
/// let mut cx = std::task::Context::from_waker(&waker);
/// match future.as_mut().poll(&mut cx) {
///     std::task::Poll::Ready(Err(payload)) => {
///         assert!(payload.downcast_ref::<&str>().is_some());
///     }
///     other => panic!("expected ready Err(payload), got {other:?}"),
/// }
/// ```
pub fn catch_unwind_future<F>(inner: F) -> CatchUnwindFuture<F>
where
    F: Future,
{
    CatchUnwindFuture::new(inner)
}

#[cfg(test)]
mod tests {
    //! `drop_guarded` reports a panicking destructor instead of unwinding.

    use super::drop_guarded;

    /// A value whose destructor panics, for the tests that need one.
    struct Detonates;

    impl Drop for Detonates {
        fn drop(&mut self) {
            panic!("detonator");
        }
    }

    /// The ordinary path returns nothing and drops the value exactly once.
    ///
    /// The `Drop` counter is what makes this more than a formality: a guard that
    /// leaked the value rather than dropping it would satisfy "no panic" while
    /// silently retaining a step's returned value for the rest of the run.
    #[test]
    fn a_quiet_drop_returns_nothing_and_runs_the_destructor() {
        use std::sync::atomic::{AtomicUsize, Ordering};

        static DROPS: AtomicUsize = AtomicUsize::new(0);

        struct Counts;
        impl Drop for Counts {
            fn drop(&mut self) { DROPS.fetch_add(1, Ordering::SeqCst); }
        }

        assert!(drop_guarded(Counts).is_none());
        assert_eq!(DROPS.load(Ordering::SeqCst), 1, "the value must be dropped");
    }

    /// A panicking destructor is reported, not propagated.
    ///
    /// This is the whole point of the guard: the call returns, and the panic's
    /// own message is what comes back rather than a type name or a placeholder.
    #[test]
    fn a_panicking_destructor_is_reported_rather_than_unwound() {
        assert_eq!(drop_guarded(Detonates).as_deref(), Some("detonator"));
    }

    /// The guard is not a licence to swallow some *other* panic.
    ///
    /// A closure body that panicked before dropping would be a different defect
    /// entirely, and the guard would report it as a drop failure — which is why
    /// the value is only ever dropped, never computed, inside the boundary.
    #[test]
    fn a_drop_that_does_not_panic_reports_nothing() {
        assert!(drop_guarded(Box::new(String::from("quiet"))).is_none());
        assert!(drop_guarded(Option::<u32>::None).is_none());
    }
}
