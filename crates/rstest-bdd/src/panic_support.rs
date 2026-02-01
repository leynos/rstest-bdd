//! Helpers for rendering panic payloads.

use std::any::Any;
use std::future::Future;
use std::pin::Pin;
use std::task::{Context, Poll};

use crate::localization;

/// Extracts a panic payload into a human-readable message.
///
/// Attempts to downcast common primitives before falling back to an opaque
/// description that includes the payload [`TypeId`].
///
/// # Examples
/// ```
/// use rstest_bdd::panic_message;
///
/// let err = std::panic::catch_unwind(|| panic!("boom"))
///     .expect_err("expected panic");
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
    pub fn new(inner: F) -> Self {
        Self(Box::pin(inner))
    }
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
