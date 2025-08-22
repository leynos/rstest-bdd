//! Core library for `rstest-bdd`.
//! This crate exposes helper utilities used by behaviour tests. It also defines
//! the global step registry used to orchestrate behaviour-driven tests.

/// Returns a greeting for the library.
///
/// # Examples
///
/// ```
/// use rstest_bdd::greet;
///
/// assert_eq!(greet(), "Hello from rstest-bdd!");
/// ```
#[must_use]
pub fn greet() -> &'static str { "Hello from rstest-bdd!" }

pub use inventory::{iter, submit};

mod context;
mod pattern;
mod placeholder;
mod registry;
mod types;

pub use context::StepContext;
pub use pattern::StepPattern;
pub use placeholder::extract_placeholders;
pub use registry::{find_step, lookup_step, Step};
pub use types::{PatternStr, PlaceholderError, StepFn, StepKeyword, StepKeywordParseError, StepText};

#[cfg(test)]
mod internal_tests;

