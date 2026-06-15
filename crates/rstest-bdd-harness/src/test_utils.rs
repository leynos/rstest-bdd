//! Shared test helpers for `rstest-bdd-harness`.

#[cfg(test)]
use std::any::Any;

/// Panic message used by `StdHarness` panic-propagation tests.
#[cfg(test)]
pub(crate) const STD_HARNESS_PANIC_MESSAGE: &str = "std harness panic propagation";

/// Returns true when a panic payload matches the expected message.
#[cfg(test)]
#[must_use]
pub(crate) fn panic_payload_matches(payload: &(dyn Any + Send), expected: &str) -> bool {
    payload
        .downcast_ref::<&str>()
        .is_some_and(|message| *message == expected)
        || payload
            .downcast_ref::<String>()
            .is_some_and(|message| message == expected)
}

/// A harness whose `run` always returns
/// `HarnessError::RuntimeBuildFailed` with a synthetic IO error, for use in
/// test suites that verify error-path behaviour.
#[cfg(feature = "testing")]
#[derive(Default)]
pub struct FailingHarness;

#[cfg(feature = "testing")]
impl crate::HarnessAdapter for FailingHarness {
    type Context = ();

    fn run<T>(
        &self,
        _request: crate::ScenarioRunRequest<'_, Self::Context, T>,
    ) -> crate::HarnessResult<T> {
        Err(crate::HarnessError::RuntimeBuildFailed(
            std::io::Error::other("synthetic harness initialisation failure"),
        ))
    }
}
