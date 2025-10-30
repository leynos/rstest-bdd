//! Helpers for signalling that a scenario should be skipped.
//!
//! The [`skip!`](crate::skip!) macro triggers a panic carrying a [`SkipRequest`]
//! payload. Step wrappers intercept that panic, convert it into a skipped
//! outcome, and stop executing subsequent steps. When the `fail_on_skipped`
//! configuration flag is enabled scenarios without an `@allow_skipped` tag
//! panic after the final step instead of being marked as skipped.

use std::fmt;
use std::panic;

/// Internal marker carried by the panic that requests the scenario to be
/// skipped.
#[derive(Debug)]
pub struct SkipRequest {
    message: Option<String>,
}

impl SkipRequest {
    /// Create a new skip request with an optional message.
    #[must_use]
    pub fn new(message: Option<String>) -> Self {
        Self { message }
    }

    /// Consume the request, returning the original message.
    #[must_use]
    pub fn into_message(self) -> Option<String> {
        self.message
    }

    /// Panic with this skip request.
    #[track_caller]
    pub fn raise(message: Option<String>) -> ! {
        panic::resume_unwind(Box::new(Self::new(message)));
    }
}

impl fmt::Display for SkipRequest {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match &self.message {
            Some(msg) => f.write_str(msg),
            None => f.write_str("scenario skipped"),
        }
    }
}

/// Panic with a [`SkipRequest`] payload to indicate the current scenario should
/// be skipped.
///
/// This function underpins the [`skip!`](crate::skip!) macro and is intentionally
/// public so behavioural tests can trigger skips without importing the macro.
#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn request_skip_raises_panic() {
        let result = panic::catch_unwind(|| SkipRequest::raise(Some("skip".to_string())));
        assert!(result.is_err(), "request_skip should panic");
    }

    #[test]
    fn skip_macro_without_message_propagates_none() {
        let result = panic::catch_unwind(|| crate::skip!());
        let Err(payload) = result else {
            panic!("skip! should raise a panic payload");
        };
        let Ok(request) = payload.downcast::<SkipRequest>() else {
            panic!("payload should downcast to SkipRequest");
        };
        assert!(
            request.into_message().is_none(),
            "skip! without arguments should produce no message",
        );
    }

    #[test]
    fn skip_macro_formats_message_arguments() {
        let detail = "service";
        let result = panic::catch_unwind(|| crate::skip!("{detail} pending", detail = detail));
        let Err(payload) = result else {
            panic!("skip! should raise a panic payload");
        };
        let Ok(request) = payload.downcast::<SkipRequest>() else {
            panic!("payload should downcast to SkipRequest");
        };
        assert_eq!(
            request.into_message(),
            Some(String::from("service pending")),
            "skip! should format the message using the provided arguments",
        );
    }
}
