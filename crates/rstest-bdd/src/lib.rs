//! Core library for `rstest-bdd`.
//! This crate exposes helper utilities used by behaviour tests. It also defines
//! the global step registry used to orchestrate behaviour-driven tests.

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
#[cfg(feature = "diagnostics")]
pub use registry::dump_registry;
pub use registry::record_bypassed_steps;
pub use registry::record_bypassed_steps_with_tags;
pub use registry::{
    Step, duplicate_steps, find_step, find_step_with_metadata, lookup_step, unused_steps,
};

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

#[doc(hidden)]
#[must_use]
pub fn __rstest_bdd_payload_from_value<T: std::any::Any>(
    value: T,
) -> Option<Box<dyn std::any::Any>> {
    if std::any::TypeId::of::<T>() == std::any::TypeId::of::<()>() {
        None
    } else {
        Some(Box::new(value) as Box<dyn std::any::Any>)
    }
}

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

/// Convenient alias for fallible step return values.
///
/// The `#[given]`, `#[when]`, and `#[then]` macros recognise this alias when
/// determining whether a step returns a `Result<..>` or a payload value.
pub type StepResult<T, E = StepError> = Result<T, E>;
