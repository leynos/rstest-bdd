//! Mode-aware async step lookup helpers.
//!
//! The core registry APIs return either sync (`StepFn`) or async (`AsyncStepFn`)
//! handlers. These helpers include execution mode metadata so callers can make
//! efficient runtime decisions (for example, preferring the sync handler for
//! synchronous steps even in async scenarios).
//!
//! Each helper delegates to the metadata pair rather than resolving a step
//! itself, so the usage-marking that [`super::ResolvedStep`] performs on
//! construction applies here without a second call to `mark_used`.

use super::StepExecutionMode;
use crate::types::{AsyncStepFn, PatternStr, StepKeyword, StepText};

/// Look up a registered async step by keyword and pattern, including its execution mode.
///
/// # Errors
///
/// Returns [`super::StepLookupError`] when equally specific definitions match.
///
/// # Examples
///
/// ```rust,ignore
/// use rstest_bdd::{StepExecutionMode, StepKeyword};
///
/// // Assume a step has been registered for this keyword/pattern.
/// let step = rstest_bdd::lookup_step_with_metadata(
///     StepKeyword::Given,
///     "some step pattern".into(),
/// )
/// .expect("lookup is unambiguous")
/// .expect("step is registered");
/// let (handler, mode) = (step.run_async, step.execution_mode);
///
/// // `handler` is the async wrapper, and `mode` tells the runtime whether the
/// // step has a native sync body, native async body, or both.
/// assert!(matches!(mode, StepExecutionMode::Sync | StepExecutionMode::Async | StepExecutionMode::Both));
/// ```
pub fn lookup_step_async_with_mode(
    keyword: StepKeyword,
    pattern: PatternStr<'_>,
) -> Result<Option<(AsyncStepFn, StepExecutionMode)>, super::StepLookupError> {
    super::lookup_step_with_metadata(keyword, pattern)
        .map(|step| step.map(|found| (found.run_async, found.execution_mode)))
}

/// Find a registered async step whose pattern matches the provided text, including its execution
/// mode.
///
/// # Errors
///
/// Returns [`super::StepLookupError`] when equally specific definitions match.
///
/// # Examples
///
/// ```rust,ignore
/// use rstest_bdd::{StepExecutionMode, StepKeyword, StepText};
///
/// // Assume a step has been registered with a pattern that matches the text.
/// let step = rstest_bdd::find_step_with_metadata(
///     StepKeyword::When,
///     StepText::from("some matching step text"),
/// )
/// .expect("lookup is unambiguous")
/// .expect("a matching step exists");
/// let (handler, mode) = (step.run_async, step.execution_mode);
///
/// assert!(matches!(mode, StepExecutionMode::Sync | StepExecutionMode::Async | StepExecutionMode::Both));
/// let _future = handler(&mut rstest_bdd::StepContext::default(), "some matching step text", None, None);
/// ```
pub fn find_step_async_with_mode(
    keyword: StepKeyword,
    text: StepText<'_>,
) -> Result<Option<(AsyncStepFn, StepExecutionMode)>, super::StepLookupError> {
    super::find_step_with_metadata(keyword, text)
        .map(|step| step.map(|found| (found.run_async, found.execution_mode)))
}

/// Find a registered step and return its full metadata, including execution mode.
///
/// This projects [`super::find_step_with_metadata`] for call sites that prefer
/// a name aligned with runtime mode selection.
///
/// # Errors
///
/// Returns [`super::StepLookupError`] when equally specific definitions match.
///
/// # Examples
///
/// ```rust,ignore
/// use rstest_bdd::{StepKeyword, StepText};
///
/// // Assume a step has been registered whose pattern matches the provided text.
/// let step = rstest_bdd::find_step_with_mode(
///     StepKeyword::Then,
///     StepText::from("some matching step text"),
/// )
/// .expect("lookup is unambiguous")
/// .expect("a matching step exists");
///
/// // `step.execution_mode` can be used to choose the most efficient execution path.
/// let _mode = step.execution_mode;
/// ```
pub fn find_step_with_mode(
    keyword: StepKeyword,
    text: StepText<'_>,
) -> Result<Option<&'static super::Step>, super::StepLookupError> {
    super::find_step_with_metadata(keyword, text).map(|step| step.map(|found| found.as_step()))
}
