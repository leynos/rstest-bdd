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
/// .expect("step is registered");
///
/// // `handler` is the async wrapper, and `mode` tells the runtime whether the
/// // step has a native sync body, native async body, or both.
/// assert!(matches!(mode, StepExecutionMode::Sync | StepExecutionMode::Async | StepExecutionMode::Both));
/// ```
#[must_use]
pub fn lookup_step_async_with_mode(
    keyword: StepKeyword,
    pattern: PatternStr<'_>,
) -> Option<(AsyncStepFn, StepExecutionMode)> {
    super::resolve_exact_step(keyword, pattern).map(|step| {
        super::mark_used((step.keyword, step.pattern));
        (step.run_async, step.execution_mode)
    })
}

/// Find a registered async step whose pattern matches the provided text, including its execution mode.
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
/// .expect("a matching step exists");
///
/// assert!(matches!(mode, StepExecutionMode::Sync | StepExecutionMode::Async | StepExecutionMode::Both));
/// let _future = handler(&mut rstest_bdd::StepContext::default(), "some matching step text", None, None);
/// ```
#[must_use]
pub fn find_step_async_with_mode(
    keyword: StepKeyword,
    text: StepText<'_>,
) -> Option<(AsyncStepFn, StepExecutionMode)> {
    super::resolve_step(keyword, text).map(|step| {
        super::mark_used((step.keyword, step.pattern));
        (step.run_async, step.execution_mode)
    })
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
/// .expect("a matching step exists");
///
/// // `step.execution_mode` can be used to choose the most efficient execution path.
/// let _mode = step.execution_mode;
/// ```
#[must_use]
pub fn find_step_with_mode(keyword: StepKeyword, text: StepText<'_>) -> Option<&'static Step> {
    super::find_step_with_metadata(keyword, text)
}
