//! Field projections from resolved registry metadata.
//!
//! Each helper delegates to [`super::lookup_step_with_metadata`] or
//! [`super::find_step_with_metadata`] and projects one field from the resolved
//! step: `run` for the synchronous helpers, `run_async` for the asynchronous
//! ones, and `(run_async, execution_mode)` for the mode-aware pair.
//!
//! Delegation is what keeps usage tracking structural. [`super::ResolvedStep`]
//! records usage on construction, so a projection inherits that behaviour
//! instead of repeating a `mark_used` call that a future variant could forget.
//! There is exactly one place a lookup touches usage tracking, and it is the
//! wrapper's constructor.
//!
//! Every helper preserves [`super::StepLookupError`] rather than collapsing it
//! to `None`. A tie between equally specific definitions means the scenario
//! selected an under-specified vocabulary, and reporting it is the whole point
//! of the scoped lookup model; discarding it would silently run a step from a
//! library the scenario never named. `Ok(None)` therefore means "nothing
//! matched", which every caller handles as a distinct case.

use crate::types::{AsyncStepFn, PatternStr, StepFn, StepKeyword, StepText};

/// Look up a registered step by keyword and pattern.
///
/// # Errors
///
/// Returns [`super::StepLookupError`] when equally specific definitions match.
pub fn lookup_step(
    keyword: StepKeyword,
    pattern: PatternStr<'_>,
) -> Result<Option<StepFn>, super::StepLookupError> {
    super::lookup_step_with_metadata(keyword, pattern).map(|step| step.map(|found| found.run))
}

/// Find a registered step whose pattern matches the provided text.
///
/// # Errors
///
/// Returns [`super::StepLookupError`] when equally specific definitions match.
pub fn find_step(
    keyword: StepKeyword,
    text: StepText<'_>,
) -> Result<Option<StepFn>, super::StepLookupError> {
    super::find_step_with_metadata(keyword, text).map(|step| step.map(|found| found.run))
}

/// Look up a registered async step by keyword and pattern.
///
/// Returns the async step function pointer for use in async scenario execution.
/// The async wrapper returns an immediately-ready future for sync step
/// definitions.
///
/// # Errors
///
/// Returns [`super::StepLookupError`] when equally specific definitions match.
pub fn lookup_step_async(
    keyword: StepKeyword,
    pattern: PatternStr<'_>,
) -> Result<Option<AsyncStepFn>, super::StepLookupError> {
    super::lookup_step_with_metadata(keyword, pattern).map(|step| step.map(|found| found.run_async))
}

/// Find a registered async step whose pattern matches the provided text.
///
/// Returns the async step function pointer for use in async scenario execution.
/// The async wrapper returns an immediately-ready future for sync step
/// definitions.
///
/// # Errors
///
/// Returns [`super::StepLookupError`] when equally specific definitions match.
pub fn find_step_async(
    keyword: StepKeyword,
    text: StepText<'_>,
) -> Result<Option<AsyncStepFn>, super::StepLookupError> {
    super::find_step_with_metadata(keyword, text).map(|step| step.map(|found| found.run_async))
}
