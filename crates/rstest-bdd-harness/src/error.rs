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
