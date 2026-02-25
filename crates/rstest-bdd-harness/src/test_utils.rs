//! Shared test helpers for `rstest-bdd-harness`.

use std::any::Any;

/// Panic message used by `StdHarness` panic-propagation tests.
pub(crate) const STD_HARNESS_PANIC_MESSAGE: &str = "std harness panic propagation";

/// Returns true when a panic payload matches the expected message.
pub(crate) fn panic_payload_matches(payload: &(dyn Any + Send), expected: &str) -> bool {
    payload
        .downcast_ref::<&str>()
        .is_some_and(|message| *message == expected)
        || payload
            .downcast_ref::<String>()
            .is_some_and(|message| message == expected)
}
