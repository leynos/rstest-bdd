//! Async step lookup helpers.
//!
//! The core registry APIs return either sync (`StepFn`) or async (`AsyncStepFn`)
//! handlers. These helpers include execution mode metadata so callers can make
//! efficient runtime decisions (for example, preferring the sync handler for
//! synchronous steps even in async scenarios).

use super::{Step, StepExecutionMode};
use crate::types::{AsyncStepFn, PatternStr, StepKeyword, StepText};

/// Look up a registered async step by keyword and pattern, including its execution mode.
///
/// # Examples
///
/// ```rust,ignore
/// use rstest_bdd::{StepExecutionMode, StepKeyword};
///
/// // Assume a step has been registered for this keyword/pattern.
/// let (handler, mode) = rstest_bdd::lookup_step_async_with_mode(
///     StepKeyword::Given,
///     "some step pattern".into(),
/// )
/// .expect("lookup is unambiguous")
/// .expect("step is registered");
///
/// // `handler` is the async wrapper, and `mode` tells the runtime whether the
/// // step has a native sync body, native async body, or both.
/// assert!(matches!(mode, StepExecutionMode::Sync | StepExecutionMode::Async | StepExecutionMode::Both));
/// ```
///
/// # Errors
///
/// Returns [`super::StepLookupError`] when equally specific definitions match.
pub fn lookup_step_async_with_mode(
    keyword: StepKeyword,
    pattern: PatternStr<'_>,
) -> Result<Option<(AsyncStepFn, StepExecutionMode)>, super::StepLookupError> {
    super::resolve_exact_step(super::StepScope::global(), keyword, pattern)
        .map(|step| super::mark_and_project(step, |found| (found.run_async, found.execution_mode)))
}

/// Find a registered async step whose pattern matches the provided text, including its execution
/// mode.
///
/// # Examples
///
/// ```rust,ignore
/// use rstest_bdd::{StepExecutionMode, StepKeyword, StepText};
///
/// // Assume a step has been registered with a pattern that matches the text.
/// let (handler, mode) = rstest_bdd::find_step_async_with_mode(
///     StepKeyword::When,
///     StepText::from("some matching step text"),
/// )
/// .expect("lookup is unambiguous")
/// .expect("a matching step exists");
///
/// assert!(matches!(mode, StepExecutionMode::Sync | StepExecutionMode::Async | StepExecutionMode::Both));
/// let _future = handler(&mut rstest_bdd::StepContext::default(), "some matching step text", None, None);
/// ```
///
/// # Errors
///
/// Returns [`super::StepLookupError`] when equally specific definitions match.
pub fn find_step_async_with_mode(
    keyword: StepKeyword,
    text: StepText<'_>,
) -> Result<Option<(AsyncStepFn, StepExecutionMode)>, super::StepLookupError> {
    super::resolve_step(super::StepScope::global(), keyword, text)
        .map(|step| super::mark_and_project(step, |found| (found.run_async, found.execution_mode)))
}

/// Find a registered step and return its full metadata, including execution mode.
///
/// This is an alias of [`super::find_step_with_metadata`] retained for call
/// sites that prefer a name aligned with runtime mode selection.
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
///
/// # Errors
///
/// Returns [`super::StepLookupError`] when equally specific definitions match.
pub fn find_step_with_mode(
    keyword: StepKeyword,
    text: StepText<'_>,
) -> Result<Option<&'static Step>, super::StepLookupError> {
    super::find_step_with_metadata(keyword, text)
}
