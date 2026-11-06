//! Clippy UI fixture for the deprecated registry lookup compatibility helpers.
//!
//! Each helper is called under its own `#[expect(deprecated, ...)]`, so an
//! unfulfilled expectation fails the `-D warnings` run. That proves every
//! helper still carries its deprecation attribute, independently of the
//! others.

#![deny(warnings)]

// The import paths are themselves deprecated, so the expectation covers the
// whole use tree; each call site below carries its own expectation.
#[expect(deprecated, reason = "imports the deprecated lookup helpers")]
use rstest_bdd::{
    StepKeyword,
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

    #[expect(deprecated, reason = "checks lookup_step deprecation")]
    let _ = lookup_step(keyword, pattern.into());
    #[expect(deprecated, reason = "checks lookup_step_async deprecation")]
    let _ = lookup_step_async(keyword, pattern.into());
    #[expect(deprecated, reason = "checks lookup_step_async_with_mode deprecation")]
    let _ = lookup_step_async_with_mode(keyword, pattern.into());
    #[expect(deprecated, reason = "checks find_step deprecation")]
    let _ = find_step(keyword, text);
    #[expect(deprecated, reason = "checks find_step_async deprecation")]
    let _ = find_step_async(keyword, text);
    #[expect(deprecated, reason = "checks find_step_async_with_mode deprecation")]
    let _ = find_step_async_with_mode(keyword, text);
    #[expect(deprecated, reason = "checks find_step_with_mode deprecation")]
    let _ = find_step_with_mode(keyword, text);
}