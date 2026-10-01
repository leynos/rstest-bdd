//! Deprecated projections from resolved registry metadata.

use crate::types::{AsyncStepFn, PatternStr, StepFn, StepKeyword, StepText};

/// Look up a registered step by keyword and pattern.
#[deprecated(
    since = "0.7.0",
    note = "Use lookup_step_with_metadata/find_step_with_metadata and project the field"
)]
#[must_use]
pub fn lookup_step(keyword: StepKeyword, pattern: PatternStr<'_>) -> Option<StepFn> {
    super::lookup_step_with_metadata(keyword, pattern).map(|step| step.run)
}

/// Find a registered step whose pattern matches the provided text.
#[deprecated(
    since = "0.7.0",
    note = "Use lookup_step_with_metadata/find_step_with_metadata and project the field"
)]
#[must_use]
pub fn find_step(keyword: StepKeyword, text: StepText<'_>) -> Option<StepFn> {
    super::find_step_with_metadata(keyword, text).map(|step| step.run)
}

/// Look up a registered async step by keyword and pattern.
///
/// Returns the async step function pointer for use in async scenario execution.
/// The async wrapper returns an immediately-ready future for sync step definitions.
#[deprecated(
    since = "0.7.0",
    note = "Use lookup_step_with_metadata/find_step_with_metadata and project the field"
)]
#[must_use]
pub fn lookup_step_async(keyword: StepKeyword, pattern: PatternStr<'_>) -> Option<AsyncStepFn> {
    super::lookup_step_with_metadata(keyword, pattern).map(|step| step.run_async)
}

/// Find a registered async step whose pattern matches the provided text.
///
/// Returns the async step function pointer for use in async scenario execution.
/// The async wrapper returns an immediately-ready future for sync step definitions.
#[deprecated(
    since = "0.7.0",
    note = "Use lookup_step_with_metadata/find_step_with_metadata and project the field"
)]
#[must_use]
pub fn find_step_async(keyword: StepKeyword, text: StepText<'_>) -> Option<AsyncStepFn> {
    super::find_step_with_metadata(keyword, text).map(|step| step.run_async)
}
