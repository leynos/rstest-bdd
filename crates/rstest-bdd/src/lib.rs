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

use ctor::ctor;
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
pub use registry::dump_registry;
pub use registry::{Step, duplicate_steps, find_step, lookup_step, unused_steps};
pub use types::{
    PatternStr, PlaceholderError, PlaceholderSyntaxError, StepFn, StepKeyword,
    StepKeywordParseError, StepPatternError, StepText,
};

#[ctor]
fn dump_steps() {
    if std::env::args().any(|a| a == "--dump-steps") {
        #[expect(
            clippy::print_stdout,
            clippy::print_stderr,
            reason = "registry dump is written to standard streams"
        )]
        {
            match dump_registry() {
                Ok(json) => println!("{json}"),
                Err(e) => eprintln!("failed to serialise step registry: {e}"),
            }
        }
        std::process::exit(0);
    }
}

/// Extracts a panic payload into a human-readable message.
///
/// Attempts to downcast common primitives before falling back to the
/// `Debug` representation of the payload.
///
/// # Examples
/// ```
/// use rstest_bdd::panic_message;
///
/// let err = std::panic::catch_unwind(|| panic!("boom"))
///     .expect_err("expected panic");
/// assert_eq!(panic_message(err.as_ref()), "boom");
/// ```
pub fn panic_message(e: &(dyn std::any::Any + Send)) -> String {
    macro_rules! try_downcast {
        ($($ty:ty),* $(,)?) => {
            $(
                if let Some(val) = e.downcast_ref::<$ty>() {
                    return val.to_string();
                }
            )*
        };
    }

    try_downcast!(&str, String, i32, u32, i64, u64, isize, usize, f32, f64);
    let ty = std::any::type_name_of_val(e);
    format!("<non-debug panic payload of type {ty}>")
}

/// Error type produced by step wrappers.
///
/// The variants categorize the possible failure modes when invoking a step.
#[derive(Debug, Error, Clone, PartialEq, Eq)]
#[non_exhaustive]
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
/// `Result<(), E: std::fmt::Display>` for explicit failure. This trait
/// normalises both forms into a `Result<(), String>` for wrapper
/// processing.
///
/// # Examples
/// ```
/// # use rstest_bdd::IntoStepResult;
/// let ok: Result<(), &str> = Ok(());
/// assert!(ok.into_step_result().is_ok());
///
/// let err: Result<(), &str> = Err("boom");
/// let res = err.into_step_result();
/// assert_eq!(res.unwrap_err(), "boom");
/// ```
pub trait IntoStepResult {
    /// Convert the value into a `Result` understood by the wrapper.
    ///
    /// # Errors
    ///
    /// Returns any error produced by the step function as a `String`.
    fn into_step_result(self) -> Result<(), String>;
}

impl IntoStepResult for () {
    fn into_step_result(self) -> Result<(), String> {
        Ok(())
    }
}

impl<E> IntoStepResult for Result<(), E>
where
    E: std::fmt::Display,
{
    fn into_step_result(self) -> Result<(), String> {
        self.map_err(|e| e.to_string())
    }
}

#[cfg(test)]
mod internal_tests;
