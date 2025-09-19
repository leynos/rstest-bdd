#![feature(auto_traits, negative_impls)]
//! Core library for `rstest-bdd`.
//!
//! ⚠️ This crate currently requires the Rust nightly compiler because it
//! relies on auto traits and negative impls to normalise step return values.
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
/// Step functions either produce no value (`()`, `Result<(), E>`) or a typed
/// value (e.g., `i32`). All forms are normalised to
/// `Result<Option<Box<dyn std::any::Any>>, String>`, where `Ok(None)` means no
/// value was produced and `Ok(Some(..))` carries the payload for later steps.
///
/// The trait uses auto-trait guards to provide distinct behaviours:
/// - `()` returns `Ok(None)` so callers do not need to handle an empty payload.
/// - `Result<(), E>` where `E: std::fmt::Display` returns `Ok(None)` on success
///   and stringifies errors.
/// - `Result<T, E>` where `T: std::any::Any` and `E: std::fmt::Display` boxes
///   the success value and stringifies errors.
/// - Any other `T: std::any::Any` (guarded by a private auto trait to exclude
///   `Result` types) stores the payload as `Some(Box<dyn std::any::Any>)`.
/// - `Result<T, E>` where `E` lacks [`std::fmt::Display`] fails to compile
///   because the auto-trait guard withholds the blanket implementation.
///
/// # Examples
/// ```
/// # use rstest_bdd::IntoStepResult;
/// let ok: Result<(), &str> = Ok(());
/// let res = ok.into_step_result();
/// assert!(matches!(res, Ok(None)));
///
/// let err: Result<(), &str> = Err("boom");
/// assert_eq!(err.into_step_result().unwrap_err(), "boom");
/// ```
///
/// Result types with non-displayable errors fail to compile:
/// ```compile_fail
/// # use rstest_bdd::IntoStepResult;
/// struct NoDisplay;
/// let res: Result<(), NoDisplay> = Err(NoDisplay);
/// let _ = res.into_step_result();
/// ```
#[doc(hidden)]
pub(crate) auto trait NotResult {}

impl<T, E> !NotResult for Result<T, E> {}

#[doc(hidden)]
pub(crate) auto trait NotUnit {}

impl !NotUnit for () {}

pub trait IntoStepResult {
    /// Convert the value into a `Result` understood by the wrapper.
    ///
    /// # Errors
    ///
    /// Returns any error produced by the step function as a `String`.
    fn into_step_result(self) -> Result<Option<Box<dyn std::any::Any>>, String>;
}

/// Default conversion for values that are neither `()` nor `Result`.
///
/// This implementation applies to all `T: std::any::Any` that are not
/// `Result` types, enforced via a private auto trait.
impl<T: std::any::Any + NotResult + NotUnit> IntoStepResult for T {
    fn into_step_result(self) -> Result<Option<Box<dyn std::any::Any>>, String> {
        Ok(Some(Box::new(self) as Box<dyn std::any::Any>))
    }
}

/// Specialisation for unit values to avoid allocating an empty payload box.
impl IntoStepResult for () {
    fn into_step_result(self) -> Result<Option<Box<dyn std::any::Any>>, String> {
        Ok(None)
    }
}

/// Implementation for `Result<(), E>` that normalises success to `Ok(None)`.
impl<E: std::fmt::Display> IntoStepResult for Result<(), E> {
    fn into_step_result(self) -> Result<Option<Box<dyn std::any::Any>>, String> {
        self.map(|()| None).map_err(|e| e.to_string())
    }
}

/// Implementation for `Result<T, E>` that boxes successful values and
/// stringifies errors.
impl<T: std::any::Any + NotUnit, E: std::fmt::Display> IntoStepResult for Result<T, E> {
    fn into_step_result(self) -> Result<Option<Box<dyn std::any::Any>>, String> {
        self.map(|value| Some(Box::new(value) as Box<dyn std::any::Any>))
            .map_err(|e| e.to_string())
    }
}

#[cfg(test)]
mod internal_tests;
