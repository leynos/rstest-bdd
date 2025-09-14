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

#[cfg(feature = "diagnostics")]
use ctor::ctor;
pub use inventory::{iter, submit};
use thiserror::Error;

mod context;
mod pattern;
mod placeholder;
mod registry;
mod types;

/// Assert that a [`Result`] is `Ok` and unwrap it.
///
/// Panics with a message including the error when the value is an `Err`.
///
/// Note: Formatting the error in the panic message requires the error type to
/// implement [`std::fmt::Display`].
///
/// # Examples
/// ```
/// use rstest_bdd::assert_step_ok;
///
/// let res: Result<(), &str> = Ok(());
/// assert_step_ok!(res);
/// ```
#[macro_export]
macro_rules! assert_step_ok {
    ($expr:expr $(,)?) => {
        match $expr {
            Ok(value) => value,
            Err(e) => panic!("step returned error: {e}"),
        }
    };
}

/// Assert that a [`Result`] is `Err` and unwrap the error.
///
/// Optionally asserts that the error's display contains a substring.
///
/// Note: The `(expr, "substring")` form requires the error type to
/// implement [`std::fmt::Display`] so it can be converted to a string for
/// matching.
///
/// # Examples
/// ```
/// use rstest_bdd::assert_step_err;
///
/// let err: Result<(), &str> = Err("boom");
/// let e = assert_step_err!(err, "boom");
/// assert_eq!(e, "boom");
/// ```
///
/// Single-argument form:
/// ```
/// use rstest_bdd::assert_step_err;
///
/// let err: Result<(), &str> = Err("boom");
/// let e = assert_step_err!(err);
/// assert_eq!(e, "boom");
/// ```
#[macro_export]
macro_rules! assert_step_err {
    ($expr:expr $(,)?) => {
        match $expr {
            Ok(_) => panic!("step succeeded unexpectedly"),
            Err(e) => e,
        }
    };
    ($expr:expr, $msg:expr $(,)?) => {
        match $expr {
            Ok(_) => panic!("step succeeded unexpectedly"),
            Err(e) => {
                let __rstest_bdd_display = e.to_string();
                let __rstest_bdd_msg: &str = $msg.as_ref();
                assert!(
                    __rstest_bdd_display.contains(__rstest_bdd_msg),
                    "error '{display}' does not contain '{msg}'",
                    display = __rstest_bdd_display,
                    msg = __rstest_bdd_msg
                );
                e
            }
        }
    };
}

pub use context::StepContext;
pub use pattern::StepPattern;
pub use placeholder::extract_placeholders;
#[cfg(feature = "diagnostics")]
pub use registry::dump_registry;
pub use registry::{Step, duplicate_steps, find_step, lookup_step, unused_steps};
pub use types::{
    PatternStr, PlaceholderError, PlaceholderSyntaxError, StepFn, StepKeyword,
    StepKeywordParseError, StepPatternError, StepText, UnsupportedStepType,
};

#[cfg(feature = "diagnostics")]
#[ctor]
fn dump_steps() {
    // Only activate when explicitly enabled by the diagnostics runner.
    if std::env::var_os("RSTEST_BDD_DUMP_STEPS").is_some()
        && std::env::args().any(|a| a == "--dump-steps")
    {
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
