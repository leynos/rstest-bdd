//! Error types for harness adapter execution.

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
}
