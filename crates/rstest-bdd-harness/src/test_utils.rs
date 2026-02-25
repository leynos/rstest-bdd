//! Shared test helpers for `rstest-bdd-harness`.

use std::any::Any;

/// Returns true when a panic payload matches the expected message.
pub(crate) fn panic_payload_matches(payload: &(dyn Any + Send), expected: &str) -> bool {
    payload
        .downcast_ref::<&str>()
        .is_some_and(|message| *message == expected)
        || payload
            .downcast_ref::<String>()
            .is_some_and(|message| message == expected)
}
