//! Core types and error enums shared across the crate.
//!
//! The module defines lightweight wrappers for pattern and step text, the step
//! keyword enum with parsing helpers, error types, and common type aliases used
//! by the registry and runner.

use crate::localization;
use std::any::Any;
use std::borrow::Cow;
use std::fmt;

// Re-export shared keyword types from rstest-bdd-patterns.
pub use rstest_bdd_patterns::{
    StepKeyword, StepKeywordParseError, UnsupportedStepType as UnsupportedStepTypeBase,
};

/// Error raised when converting a parsed Gherkin [`gherkin::StepType`] into a
/// [`StepKeyword`] fails.
///
/// This is a localized wrapper around [`UnsupportedStepTypeBase`] that uses the
/// runtime localization system for user-friendly error messages.
///
/// # Examples
///
/// ```rust
/// use gherkin::StepType;
/// use rstest_bdd::{StepKeyword, UnsupportedStepType};
///
/// fn convert(ty: StepType) -> Result<StepKeyword, UnsupportedStepType> {
///     StepKeyword::try_from(ty).map_err(UnsupportedStepType::from)
/// }
///
/// match convert(StepType::Given) {
///     Ok(keyword) => assert_eq!(keyword, StepKeyword::Given),
///     Err(error) => {
///         eprintln!("unsupported step type: {:?}", error.0);
///     }
/// }
/// ```
#[derive(Debug)]
pub struct UnsupportedStepType(pub gherkin::StepType);

impl fmt::Display for UnsupportedStepType {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let message = localization::message_with_args("unsupported-step-type", |args| {
            args.set("step_type", format!("{:?}", self.0));
        });
        f.write_str(&message)
    }
}

impl std::error::Error for UnsupportedStepType {}

impl From<UnsupportedStepTypeBase> for UnsupportedStepType {
    fn from(base: UnsupportedStepTypeBase) -> Self {
        Self(base.0)
    }
}

/// Wrapper for step pattern strings used in matching logic.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct PatternStr<'a>(pub(crate) &'a str);

impl<'a> PatternStr<'a> {
    /// Construct a new `PatternStr` from a string slice.
    #[must_use]
    pub const fn new(s: &'a str) -> Self {
        Self(s)
    }

    /// Access the underlying string slice.
    #[must_use]
    pub const fn as_str(self) -> &'a str {
        self.0
    }
}

impl<'a> From<&'a str> for PatternStr<'a> {
    fn from(s: &'a str) -> Self {
        Self::new(s)
    }
}

/// Wrapper for step text content from scenarios.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct StepText<'a>(pub(crate) &'a str);

impl<'a> StepText<'a> {
    /// Construct a new `StepText` from a string slice.
    #[must_use]
    pub const fn new(s: &'a str) -> Self {
        Self(s)
    }

    /// Access the underlying string slice.
    #[must_use]
    pub const fn as_str(self) -> &'a str {
        self.0
    }
}

impl<'a> From<&'a str> for StepText<'a> {
    fn from(s: &'a str) -> Self {
        Self::new(s)
    }
}

/// Detailed information about placeholder parsing failures.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PlaceholderSyntaxError {
    /// Human‑readable reason for the failure.
    pub message: String,
    /// Zero-based byte offset in the original pattern where parsing failed.
    pub position: usize,
    /// Name of the placeholder, when known.
    pub placeholder: Option<String>,
}

impl PlaceholderSyntaxError {
    /// Construct a new syntax error with optional placeholder context.
    #[must_use]
    pub fn new(message: impl Into<String>, position: usize, placeholder: Option<String>) -> Self {
        Self {
            message: message.into(),
            position,
            placeholder,
        }
    }

    /// Return the user‑facing message without the "invalid placeholder syntax" prefix.
    #[must_use]
    pub fn user_message(&self) -> String {
        let suffix = self
            .placeholder
            .as_ref()
            .map(|name| {
                let detail = localization::message_with_args("placeholder-syntax-suffix", |args| {
                    args.set("placeholder", name.clone());
                });
                format!(" {detail}")
            })
            .unwrap_or_default();
        localization::message_with_args("placeholder-syntax-detail", |args| {
            args.set("reason", self.message.clone());
            args.set("position", self.position.to_string());
            args.set("suffix", suffix);
        })
    }
}

impl fmt::Display for PlaceholderSyntaxError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let message = localization::message_with_args("placeholder-syntax", |args| {
            args.set("details", self.user_message());
        });
        f.write_str(&message)
    }
}

impl std::error::Error for PlaceholderSyntaxError {}

/// Errors that may occur when compiling a [`StepPattern`].
#[derive(Debug)]
#[non_exhaustive]
pub enum StepPatternError {
    /// Placeholder syntax in the pattern is invalid.
    PlaceholderSyntax(PlaceholderSyntaxError),
    /// The generated regular expression failed to compile.
    InvalidPattern(regex::Error),
    /// Attempted to access the compiled regex before calling [`StepPattern::compile`](crate::pattern::StepPattern::compile).
    NotCompiled {
        /// Pattern text that has not yet been compiled.
        pattern: Cow<'static, str>,
    },
}

impl From<PlaceholderSyntaxError> for StepPatternError {
    fn from(err: PlaceholderSyntaxError) -> Self {
        Self::PlaceholderSyntax(err)
    }
}

impl From<regex::Error> for StepPatternError {
    fn from(err: regex::Error) -> Self {
        Self::InvalidPattern(err)
    }
}

impl fmt::Display for StepPatternError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::PlaceholderSyntax(err) => err.fmt(f),
            Self::InvalidPattern(err) => err.fmt(f),
            Self::NotCompiled { pattern } => {
                let message =
                    localization::message_with_args("step-pattern-not-compiled", |args| {
                        args.set("pattern", pattern.to_string());
                    });
                f.write_str(&message)
            }
        }
    }
}

impl std::error::Error for StepPatternError {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        match self {
            Self::PlaceholderSyntax(err) => Some(err),
            Self::InvalidPattern(err) => Some(err),
            Self::NotCompiled { .. } => None,
        }
    }
}

/// Error conditions that may arise when extracting placeholders.
#[derive(Debug, Clone, PartialEq, Eq)]
#[non_exhaustive]
pub enum PlaceholderError {
    /// The supplied text did not match the step pattern.
    PatternMismatch,
    /// The step pattern contained invalid placeholder syntax.
    InvalidPlaceholder(String),
    /// The step pattern could not be compiled into a regular expression.
    InvalidPattern(String),
    /// The step pattern regex was accessed before compilation.
    NotCompiled {
        /// Pattern text that must be compiled prior to use.
        pattern: String,
    },
}

impl fmt::Display for PlaceholderError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let message = match self {
            Self::PatternMismatch => localization::message("placeholder-pattern-mismatch"),
            Self::InvalidPlaceholder(details) => {
                localization::message_with_args("placeholder-invalid-placeholder", |args| {
                    args.set("details", details.clone());
                })
            }
            Self::InvalidPattern(pattern) => {
                localization::message_with_args("placeholder-invalid-pattern", |args| {
                    args.set("pattern", pattern.clone());
                })
            }
            Self::NotCompiled { pattern } => {
                localization::message_with_args("placeholder-not-compiled", |args| {
                    args.set("pattern", pattern.clone());
                })
            }
        };
        f.write_str(&message)
    }
}

impl std::error::Error for PlaceholderError {}

impl From<StepPatternError> for PlaceholderError {
    fn from(e: StepPatternError) -> Self {
        match e {
            StepPatternError::PlaceholderSyntax(err) => {
                Self::InvalidPlaceholder(err.user_message())
            }
            StepPatternError::InvalidPattern(err) => Self::InvalidPattern(err.to_string()),
            StepPatternError::NotCompiled { pattern } => Self::NotCompiled {
                pattern: pattern.into_owned(),
            },
        }
    }
}

/// Outcome produced by step wrappers.
#[derive(Debug)]
#[must_use]
pub enum StepExecution {
    /// The step executed successfully and may provide a value for later steps.
    Continue {
        /// Value returned by the step, made available to later fixtures.
        value: Option<Box<dyn Any>>,
    },
    /// The step requested that the scenario should be skipped.
    Skipped {
        /// Optional reason describing why execution stopped.
        message: Option<String>,
    },
}

impl StepExecution {
    /// Construct a successful outcome with an optional value.
    pub fn from_value(value: Option<Box<dyn Any>>) -> Self {
        Self::Continue { value }
    }

    /// Construct a skipped outcome with an optional reason.
    pub fn skipped(message: impl Into<Option<String>>) -> Self {
        Self::Skipped {
            message: message.into(),
        }
    }
}

/// Type alias for the stored step function pointer.
pub type StepFn = for<'a> fn(
    &mut crate::context::StepContext<'a>,
    &str,
    Option<&str>,
    Option<&[&[&str]]>,
) -> Result<StepExecution, crate::StepError>;

#[cfg(test)]
mod tests;
