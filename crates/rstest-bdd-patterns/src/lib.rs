//! Shared step-pattern parsing utilities for rstest-bdd.
//!
//! The crate exposes placeholder parsing helpers reused by both the runtime
//! and proc-macro crates so they can share validation logic without duplicating
//! the regex construction code paths.

mod capture;
mod errors;
mod hint;
mod pattern;

pub use capture::extract_captured_values;
pub use errors::{PatternError, PlaceholderErrorInfo};
pub use hint::get_type_pattern;
pub use pattern::{build_regex_from_pattern, compile_regex_from_pattern};
