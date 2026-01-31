//! Async step lookup helpers.
//!
//! The core registry APIs return either sync (`StepFn`) or async (`AsyncStepFn`)
//! handlers. These helpers include execution mode metadata so callers can make
//! efficient runtime decisions (for example, preferring the sync handler for
//! synchronous steps even in async scenarios).

use super::{Step, StepExecutionMode};
use crate::types::{AsyncStepFn, PatternStr, StepKeyword, StepText};

/// Look up a registered async step by keyword and pattern, including its execution mode.
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
#[must_use]
pub fn find_step_with_mode(keyword: StepKeyword, text: StepText<'_>) -> Option<&'static Step> {
    super::find_step_with_metadata(keyword, text)
}
