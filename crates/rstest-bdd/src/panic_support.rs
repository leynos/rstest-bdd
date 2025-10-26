//! Helpers for rendering panic payloads.

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

    try_downcast!(&str, String, i32, u32, i64, u64, isize, usize, f32, f64);
    let ty = format!(
        "erased `Any` payload (TypeId({:?}); panic with Display/Debug data for detail)",
        e.type_id()
    );
    localization::message_with_args("panic-message-opaque-payload", |args| {
        args.set("type", ty);
    })
}
