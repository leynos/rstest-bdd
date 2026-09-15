//! Async step lookup helpers.
//!
//! The core registry APIs return either sync (`StepFn`) or async (`AsyncStepFn`)
//! handlers. These helpers include execution mode metadata so callers can make
//! efficient runtime decisions (for example, preferring the sync handler for
//! synchronous steps even in async scenarios).

use super::StepExecutionMode;
use crate::types::{AsyncStepFn, PatternStr, StepKeyword, StepText};

/// Look up a registered async step by keyword and pattern, including its execution mode.
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
/// .expect("step is registered");
/// let (handler, mode) = (step.run_async, step.execution_mode);
///
/// // `handler` is the async wrapper, and `mode` tells the runtime whether the
/// // step has a native sync body, native async body, or both.
/// assert!(matches!(mode, StepExecutionMode::Sync | StepExecutionMode::Async | StepExecutionMode::Both));
/// ```
#[deprecated(
    since = "0.7.0",
    note = "Use lookup_step_with_metadata/find_step_with_metadata and project the field"
)]
#[must_use]
pub fn lookup_step_async_with_mode(
    keyword: StepKeyword,
    pattern: PatternStr<'_>,
) -> Option<(AsyncStepFn, StepExecutionMode)> {
    super::lookup_step_with_metadata(keyword, pattern)
        .map(|step| (step.run_async, step.execution_mode))
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
/// let step = rstest_bdd::find_step_with_metadata(
///     StepKeyword::When,
///     StepText::from("some matching step text"),
/// )
/// .expect("a matching step exists");
/// let (handler, mode) = (step.run_async, step.execution_mode);
///
/// assert!(matches!(mode, StepExecutionMode::Sync | StepExecutionMode::Async | StepExecutionMode::Both));
/// let _future = handler(&mut rstest_bdd::StepContext::default(), "some matching step text", None, None);
/// ```
#[deprecated(
    since = "0.7.0",
    note = "Use lookup_step_with_metadata/find_step_with_metadata and project the field"
)]
#[must_use]
pub fn find_step_async_with_mode(
    keyword: StepKeyword,
    text: StepText<'_>,
) -> Option<(AsyncStepFn, StepExecutionMode)> {
    super::find_step_with_metadata(keyword, text).map(|step| (step.run_async, step.execution_mode))
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
/// let step = rstest_bdd::find_step_with_metadata(
///     StepKeyword::Then,
///     StepText::from("some matching step text"),
/// )
/// .expect("a matching step exists");
///
/// // `step.execution_mode` can be used to choose the most efficient execution path.
/// let _mode = step.execution_mode;
/// ```
#[deprecated(since = "0.7.0", note = "Use find_step_with_metadata instead")]
#[must_use]
pub fn find_step_with_mode(
    keyword: StepKeyword,
    text: StepText<'_>,
) -> Option<&'static super::Step> {
    super::find_step_with_metadata(keyword, text).map(|step| step.as_step())
}
