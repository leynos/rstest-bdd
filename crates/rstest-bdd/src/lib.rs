//! Core library for `rstest-bdd`.
//! ⚠️ This crate currently requires the Rust nightly compiler because it
//! relies on auto traits and negative impls to normalise step return values.
//! This crate exposes helper utilities used by behaviour tests. It also defines
//! the global step registry used to orchestrate behaviour-driven tests.
#![feature(auto_traits, negative_impls)]

extern crate self as rstest_bdd;

pub mod config;
mod macros;
mod skip;

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
pub use i18n_embed::fluent::FluentLanguageLoader;
pub use inventory::{iter, submit};

mod context;
pub mod datatable;
pub mod localization;
mod panic_support;
mod pattern;
mod placeholder;
mod registry;
pub mod reporting;
mod skip_helpers;
pub mod state;
pub mod step_args;
mod types;

pub use context::{FixtureRef, FixtureRefMut, StepContext};
pub use localization::{
    LocalizationError, Localizations, current_languages, install_localization_loader,
    select_localizations,
};
pub use pattern::StepPattern;
pub use placeholder::extract_placeholders;
pub use registry::{Step, duplicate_steps, find_step, lookup_step, unused_steps};
#[cfg(feature = "diagnostics")]
pub use registry::{dump_registry, record_bypassed_steps};

/// Whether the crate was built with the `diagnostics` feature enabled.
#[must_use]
pub const fn diagnostics_enabled() -> bool {
    cfg!(feature = "diagnostics")
}
#[doc(hidden)]
pub use skip::{
    ScopeKind as __rstest_bdd_scope_kind, SkipRequest, StepScopeGuard as __rstest_bdd_scope_guard,
    enter_scope as __rstest_bdd_enter_scope,
    request_current_skip as __rstest_bdd_request_current_skip,
};
#[doc(hidden)]
pub use skip_helpers::{
    __rstest_bdd_assert_scenario_detail_flag, __rstest_bdd_assert_scenario_detail_message_absent,
    __rstest_bdd_assert_scenario_detail_message_contains,
    __rstest_bdd_assert_step_skipped_message_absent,
    __rstest_bdd_assert_step_skipped_message_contains, __rstest_bdd_expect_skip_flag,
    __rstest_bdd_expect_skip_message_absent, __rstest_bdd_expect_skip_message_contains,
};
pub use state::{ScenarioState, Slot};
pub use step_args::{StepArgs, StepArgsError};
pub use types::{
    PatternStr, PlaceholderError, PlaceholderSyntaxError, StepExecution, StepFn, StepKeyword,
    StepKeywordParseError, StepPatternError, StepText, UnsupportedStepType,
};

#[cfg(feature = "diagnostics")]
#[ctor]
fn dump_steps() {
    // Only activate when explicitly enabled by the diagnostics runner.
    if std::env::var_os("RSTEST_BDD_DUMP_STEPS").is_some()
        && std::env::args().any(|a| a == "--dump-steps")
    {
        reporting::run_dump_seeds();
        #[expect(
            clippy::print_stdout,
            clippy::print_stderr,
            reason = "registry dump is written to standard streams"
        )]
        {
            match dump_registry() {
                Ok(json) => println!("{json}"),
                Err(e) => eprintln!("failed to serialize step registry: {e}"),
            }
        }
        std::process::exit(0);
    }
}

pub use panic_support::panic_message;

/// Error type produced by step wrappers.
///
/// The variants categorize the possible failure modes when invoking a step.
#[derive(Debug, Clone, PartialEq, Eq)]
#[non_exhaustive]
pub enum StepError {
    /// Raised when a required fixture is absent from the [`StepContext`].
    MissingFixture {
        /// Name of the missing fixture.
        name: String,
        /// Type of the missing fixture.
        ty: String,
        /// Step function that requested the fixture.
        step: String,
    },
    /// Raised when the invoked step function returns an [`Err`] variant.
    ExecutionError {
        /// Pattern text used when invoking the step.
        pattern: String,
        /// Name of the step function.
        function: String,
        /// Error message produced by the step function.
        message: String,
    },
    /// Raised when the step function panics during execution.
    PanicError {
        /// Pattern text used when invoking the step.
        pattern: String,
        /// Name of the step function.
        function: String,
        /// Panic payload converted to a string.
        message: String,
    },
}

// Macro that maps `StepError` variants to their Fluent identifiers without
// repeating localization boilerplate in each match arm.
macro_rules! step_error_message {
    (
        $self:expr,
        $loader:expr,
        $( $variant:ident { $( $field:ident ),* } => $id:literal ),+ $(,)?
    ) => {{
        match $self {
            $(
                Self::$variant { $( $field ),* } => {
                    $crate::localization::message_with_loader($loader, $id, |args| {
                        $( args.set(stringify!($field), $field.clone()); )*
                    })
                }
            ),+
        }
    }};
}

impl StepError {
    /// Render the error message using the provided Fluent loader.
    ///
    /// # Examples
    /// ```
    /// # use rstest_bdd::StepError;
    /// # use rstest_bdd::localization::Localizations;
    /// # use i18n_embed::fluent::{fluent_language_loader, FluentLanguageLoader};
    /// # use unic_langid::langid;
    /// let loader: FluentLanguageLoader = {
    ///     let mut loader = fluent_language_loader!();
    ///     i18n_embed::select(&loader, &Localizations, &[langid!("en-US")]).unwrap();
    ///     loader
    /// };
    /// let error = StepError::MissingFixture {
    ///     name: "db".into(),
    ///     ty: "Pool".into(),
    ///     step: "Given a database".into(),
    /// };
    /// let message = error.format_with_loader(&loader);
    /// assert!(message.contains("Missing fixture 'db'"));
    /// ```
    #[must_use]
    pub fn format_with_loader(&self, loader: &FluentLanguageLoader) -> String {
        step_error_message!(
            self,
            loader,
            MissingFixture { name, ty, step } => "step-error-missing-fixture",
            ExecutionError { pattern, function, message } => "step-error-execution",
            PanicError { pattern, function, message } => "step-error-panic",
        )
    }
}

impl std::fmt::Display for StepError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        let message = localization::with_loader(|loader| self.format_with_loader(loader));
        f.write_str(&message)
    }
}

impl std::error::Error for StepError {}

#[doc(hidden)]
pub(crate) auto trait NotResult {}

impl<T, E> !NotResult for Result<T, E> {}

#[doc(hidden)]
pub(crate) auto trait NotUnit {}

impl !NotUnit for () {}

/// Convert step function outputs into a standard result type.
///
/// Step functions either produce no value (`()`, `Result<(), E>`) or a typed
/// value (e.g., `i32`). All forms are normalised to
/// `Result<Option<Box<dyn std::any::Any>>, String>`, where `Ok(None)` means no
/// value was produced and `Ok(Some(..))` carries the payload for later steps.
///
/// The trait uses disjoint impls selected via private auto traits and negative
/// impls to provide optimised behaviour for common return shapes:
/// - `()` has a dedicated implementation returning `Ok(None)` so callers do not
///   need to handle an empty payload.
/// - `Result<(), E>` where `E: std::fmt::Display` maps `Ok(())` to `Ok(None)`
///   whilst stringifying any error.
/// - `Result<T, E>` where `T: std::any::Any + NotUnit` and `E: std::fmt::Display`
///   boxes the success value and stringifies any error.
///
/// When none of those special cases apply, the blanket
/// `T: std::any::Any + NotResult + NotUnit` implementation acts as the default:
/// it boxes the value as `Some(Box<dyn std::any::Any>)`.
/// The private auto traits ensure that `Result<_, _>` and `()` do not match
/// this impl and instead use the dedicated ones above.
/// Error types in the `Result<_, E>` impls must implement [`std::fmt::Display`]
/// so they can be converted into strings for the wrapper.
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
