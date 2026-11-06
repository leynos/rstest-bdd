//! Deprecated projections from resolved registry metadata.
//!
//! This module is the compatibility surface for callers that still expect a
//! bare handler pointer. Each helper delegates to
//! [`super::lookup_step_with_metadata`] or [`super::find_step_with_metadata`]
//! and projects one field from the resolved step: `run` for the synchronous
//! helpers, `run_async` for the asynchronous ones, and `(run_async,
//! execution_mode)` for the mode-aware pair. Because every helper delegates to
//! a metadata lookup, it inherits the usage-marking behaviour that
//! [`super::ResolvedStep`] applies on construction.
//!
//! New code should call the metadata pair directly and project the field it
//! needs. The helpers here remain available, with deprecation warnings, until
//! the next breaking release; the v0.7.0 migration guide maps each one to its
//! replacement.

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
