//! Core library for `rstest-bdd`.
//! This crate exposes helper utilities used by behaviour tests. It also defines
//! the global step registry used to orchestrate behaviour-driven tests.

/// Returns a greeting for the library.
///
/// # Examples
///
/// ```
/// use rstest_bdd::greet;
///
/// assert_eq!(greet(), "Hello from rstest-bdd!");
/// ```
#[must_use]
pub fn greet() -> &'static str {
    "Hello from rstest-bdd!"
}

pub use inventory::{iter, submit};
use thiserror::Error;

mod context;
mod pattern;
mod placeholder;
mod registry;
mod types;

pub use context::StepContext;
pub use pattern::StepPattern;
pub use placeholder::extract_placeholders;
pub use registry::{Step, find_step, lookup_step};
pub use types::{
    PatternStr, PlaceholderError, StepFn, StepKeyword, StepKeywordParseError, StepText,
};

/// Error type produced by step wrappers.
///
/// The variants categorise the possible failure modes when invoking a step.
#[derive(Debug, Error, Clone, PartialEq)]
pub enum StepError {
    /// Raised when a required fixture is absent from the [`StepContext`].
    #[error("Missing fixture '{name}' of type '{ty}' for step function '{step}'")]
    MissingFixture {
        /// Name of the missing fixture.
        name: String,
        /// Type of the missing fixture.
        ty: String,
        /// Step function that requested the fixture.
        step: String,
    },
    /// Raised when the invoked step function returns an [`Err`] variant.
    #[error("Error executing step '{pattern}' via function '{function}': {message}")]
    ExecutionError {
        /// Pattern text used when invoking the step.
        pattern: String,
        /// Name of the step function.
        function: String,
        /// Error message produced by the step function.
        message: String,
    },
    /// Raised when the step function panics during execution.
    #[error("Panic in step '{pattern}', function '{function}': {message}")]
    PanicError {
        /// Pattern text used when invoking the step.
        pattern: String,
        /// Name of the step function.
        function: String,
        /// Panic payload converted to a string.
        message: String,
    },
}

/// Convert step function outputs into a standard result type.
///
/// Step functions may return either `()` to signal success or
/// `Result<(), String>` for explicit failure. This trait normalises both
/// forms into a `Result<(), String>` for wrapper processing.
pub trait IntoStepResult {
    /// Convert the value into a `Result` understood by the wrapper.
    ///
    /// # Errors
    ///
    /// Returns any error produced by the step function as a `String`.
    fn into_step_result(self) -> Result<(), String>;
}

impl IntoStepResult for () {
    fn into_step_result(self) -> Result<(), String> { Ok(()) }
}

impl IntoStepResult for Result<(), String> {
    fn into_step_result(self) -> Result<(), String> { self }
}

#[cfg(test)]
mod internal_tests;
