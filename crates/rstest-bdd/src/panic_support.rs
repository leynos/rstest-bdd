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
pub struct CatchUnwindFuture<F>(Pin<Box<F>>);

impl<F> CatchUnwindFuture<F> {
    /// Wrap the provided future.
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
#[doc(hidden)]
pub fn catch_unwind_future<F>(inner: F) -> CatchUnwindFuture<F>
where
    F: Future,
{
    CatchUnwindFuture::new(inner)
}
