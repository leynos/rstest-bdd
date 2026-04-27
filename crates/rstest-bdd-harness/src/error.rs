//! Error types for harness adapter execution.

use std::fmt;

/// Convenience alias for harness adapter results.
pub type HarnessResult<T> = Result<T, HarnessError>;

/// Errors returned by [`crate::HarnessAdapter`] implementations.
///
/// Harness adapters use this enum to report infrastructure failures that occur
/// before or around scenario execution, such as runtime initialization
/// problems.
#[derive(Debug, thiserror::Error)]
#[non_exhaustive]
pub enum HarnessError {
    /// Building the harness runtime failed before the scenario could run.
    #[error("failed to build runtime: {0}")]
    RuntimeBuildFailed(#[source] std::io::Error),
}

/// A harness error annotated with the scenario that was being initialized.
///
/// This wrapper keeps [`HarnessError`] variants directly matchable while
/// giving generated tests and harness adapters a structured value that carries
/// feature and scenario context for logging or panic messages.
#[derive(Debug)]
pub struct HarnessErrorContext {
    error: HarnessError,
    feature_path: String,
    scenario_name: String,
}

impl HarnessErrorContext {
    /// Creates a context-bearing harness error.
    #[must_use]
    pub fn new(
        error: HarnessError,
        feature_path: impl Into<String>,
        scenario_name: impl Into<String>,
    ) -> Self {
        Self {
            error,
            feature_path: feature_path.into(),
            scenario_name: scenario_name.into(),
        }
    }

    /// Returns the original typed harness error.
    #[must_use]
    pub const fn error(&self) -> &HarnessError {
        &self.error
    }

    /// Returns the feature path for the scenario that failed to initialize.
    #[must_use]
    pub fn feature_path(&self) -> &str {
        &self.feature_path
    }

    /// Returns the scenario name that failed to initialize.
    #[must_use]
    pub fn scenario_name(&self) -> &str {
        &self.scenario_name
    }

    /// Consumes this wrapper and returns the original typed error.
    #[must_use]
    pub fn into_error(self) -> HarnessError {
        self.error
    }
}

impl fmt::Display for HarnessErrorContext {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            formatter,
            "{} (feature: {}, scenario: {})",
            self.error, self.feature_path, self.scenario_name
        )
    }
}

impl std::error::Error for HarnessErrorContext {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        Some(&self.error)
    }
}

impl HarnessError {
    /// Attaches scenario context to this error for diagnostics.
    #[must_use]
    pub fn with_scenario_context(
        self,
        feature_path: impl Into<String>,
        scenario_name: impl Into<String>,
    ) -> HarnessErrorContext {
        HarnessErrorContext::new(self, feature_path, scenario_name)
    }
}

#[cfg(test)]
mod tests {
    //! Unit tests for harness error formatting and source chaining.

    use super::HarnessError;
    use std::error::Error;
    use std::io;

    #[test]
    fn runtime_build_failed_display_includes_io_error_message() {
        let err = HarnessError::RuntimeBuildFailed(io::Error::other("runtime denied"));

        assert_eq!(format!("{err}"), "failed to build runtime: runtime denied");
    }

    #[test]
    fn runtime_build_failed_exposes_io_error_source() {
        let err = HarnessError::RuntimeBuildFailed(io::Error::other("runtime denied"));

        assert!(err.source().is_some());
    }

    #[test]
    fn runtime_build_failed_display_format_is_stable() {
        let err = HarnessError::RuntimeBuildFailed(io::Error::other("known build failure"));

        assert_eq!(
            format!("{err}"),
            "failed to build runtime: known build failure"
        );
    }

    #[test]
    fn harness_error_context_display_includes_scenario_metadata() {
        let err = HarnessError::RuntimeBuildFailed(io::Error::other("runtime denied"))
            .with_scenario_context("features/demo.feature", "Observed failure");

        assert_eq!(
            format!("{err}"),
            concat!(
                "failed to build runtime: runtime denied ",
                "(feature: features/demo.feature, scenario: Observed failure)"
            )
        );
    }

    #[test]
    fn runtime_build_failed_display_snapshot() {
        let err = HarnessError::RuntimeBuildFailed(io::Error::other("build denied"));

        insta::assert_snapshot!(format!("{err}"));
    }
}
