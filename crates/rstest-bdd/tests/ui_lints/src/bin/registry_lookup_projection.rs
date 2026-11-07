//! Clippy UI fixture for the registry lookup projection helpers.
//!
//! Each helper is called under `-D warnings` and its result bound to an
//! explicitly typed value, so this fixture pins two obligations that would
//! otherwise drift silently: every projection preserves the
//! `Result<Option<_>, StepLookupError>` shape, and every projection returns the
//! handler or metadata type its name promises. A helper that started collapsing
//! the ambiguity to `None`, or that lost its `#[must_use]`-worthy result, would
//! stop type-checking here.

#![deny(warnings)]

use rstest_bdd::{
    AsyncStepFn,
    Step,
    StepExecutionMode,
    StepFn,
    StepKeyword,
    StepLookupError,
    StepText,
    find_step,
    find_step_async,
    find_step_async_with_mode,
    find_step_with_mode,
    lookup_step,
    lookup_step_async,
    lookup_step_async_with_mode,
};

fn main() {
    let keyword = StepKeyword::Given;
    let pattern = "a registered step";
    let text = StepText::from(pattern);

    let exact: Result<Option<StepFn>, StepLookupError> = lookup_step(keyword, pattern.into());
    let fuzzy: Result<Option<StepFn>, StepLookupError> = find_step(keyword, text);

    let exact_async: Result<Option<AsyncStepFn>, StepLookupError> =
        lookup_step_async(keyword, pattern.into());
    let fuzzy_async: Result<Option<AsyncStepFn>, StepLookupError> = find_step_async(keyword, text);

    let exact_mode: Result<Option<(AsyncStepFn, StepExecutionMode)>, StepLookupError> =
        lookup_step_async_with_mode(keyword, pattern.into());
    let fuzzy_mode: Result<Option<(AsyncStepFn, StepExecutionMode)>, StepLookupError> =
        find_step_async_with_mode(keyword, text);

    let metadata: Result<Option<&'static Step>, StepLookupError> =
        find_step_with_mode(keyword, text);

    let _ = (
        exact,
        fuzzy,
        exact_async,
        fuzzy_async,
        exact_mode,
        fuzzy_mode,
        metadata,
    );
}
