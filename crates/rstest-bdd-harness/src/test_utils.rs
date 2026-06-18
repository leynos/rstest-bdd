//! Shared test helpers for `rstest-bdd-harness`.
//!
//! Provides panic-payload matching utilities for harness tests that assert
//! panic propagation. Harness adapters must re-raise scenario panics so test
//! runners report the original failure; these helpers let tests verify the
//! re-raised payload carries the expected message regardless of whether the
//! payload is a `&str` (from `panic!("literal")`) or a `String` (from
//! `panic!("{interpolated}")` or `std::panic::resume_unwind`).
//!
//! Harness implementations should use [`panic_payload_matches`] when writing
//! `catch_unwind`-based propagation tests instead of downcasting payloads
//! inline, so the `&str`/`String` asymmetry is handled in one place.

use std::any::Any;

/// Panic message used by `StdHarness` panic-propagation tests.
pub(crate) const STD_HARNESS_PANIC_MESSAGE: &str = "std harness panic propagation";

/// Returns true when a panic payload matches the expected message.
#[must_use]
pub(crate) fn panic_payload_matches(payload: &(dyn Any + Send), expected: &str) -> bool {
    payload
        .downcast_ref::<&str>()
        .is_some_and(|message| *message == expected)
        || payload
            .downcast_ref::<String>()
            .is_some_and(|message| message == expected)
}
