//! Dependency-facing harness helpers for integration tests.

/// A harness whose `run` always returns
/// [`crate::HarnessError::RuntimeBuildFailed`] with a synthetic IO error.
///
/// Downstream harness crates use this helper to verify their generated
/// scenario error paths without duplicating an adapter implementation.
#[derive(Default)]
pub struct FailingHarness;

impl crate::HarnessAdapter for FailingHarness {
    type Context = ();

    fn run<T>(
        &self,
        _request: crate::ScenarioRunRequest<'_, Self::Context, T>,
    ) -> crate::HarnessResult<T> {
        Err(crate::HarnessError::RuntimeBuildFailed(
            std::io::Error::other("synthetic harness initialization failure"),
        ))
    }
}
