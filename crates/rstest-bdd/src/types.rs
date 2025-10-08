//! Core types and error enums used across the crate.
//! This module defines the lightweight wrappers for pattern and step text,
//! the step keyword enum with parsing helpers, error types, and common type
//! aliases used by the registry and runner.

use crate::localisation;
use gherkin::StepType;
use std::borrow::Cow;
use std::fmt;
use std::str::FromStr;

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

/// Keyword used to categorize a step definition.
///
/// The enum includes `And` and `But` variants for completeness, but feature
/// parsing resolves them against the preceding `Given`/`When`/`Then`.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum StepKeyword {
    /// Setup preconditions for a scenario.
    Given,
    /// Perform an action when testing behaviour.
    When,
    /// Assert the expected outcome of a scenario.
    Then,
    /// Additional conditions that share context with the previous step.
    And,
    /// Negative or contrasting conditions.
    But,
}

impl StepKeyword {
    /// Return the keyword as a string slice.
    #[must_use]
    pub const fn as_str(&self) -> &'static str {
        match self {
            Self::Given => "Given",
            Self::When => "When",
            Self::Then => "Then",
            Self::And => "And",
            Self::But => "But",
        }
    }
}

/// Error returned when parsing a `StepKeyword` from a string fails.
#[derive(Debug)]
pub struct StepKeywordParseError(pub String);

impl fmt::Display for StepKeywordParseError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let message = localisation::message_with_args("step-keyword-parse-error", |args| {
            args.set("keyword", self.0.clone());
        });
        f.write_str(&message)
    }
}

impl std::error::Error for StepKeywordParseError {}

impl FromStr for StepKeyword {
    type Err = StepKeywordParseError;

    fn from_str(value: &str) -> Result<Self, Self::Err> {
        let kw = match value.trim().to_ascii_lowercase().as_str() {
            "given" => Self::Given,
            "when" => Self::When,
            "then" => Self::Then,
            "and" => Self::And,
            "but" => Self::But,
            other => return Err(StepKeywordParseError(other.to_string())),
        };
        Ok(kw)
    }
}

/// Error raised when converting a parsed Gherkin [`StepType`] into a
/// [`StepKeyword`] fails.
///
/// Captures the offending [`StepType`] to help callers diagnose missing
/// language support. Implements [`fmt::Display`] and [`std::error::Error`]
/// for consumption by callers via conventional error handling.
#[derive(Debug)]
pub struct UnsupportedStepType(pub StepType);

impl fmt::Display for UnsupportedStepType {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let message = localisation::message_with_args("unsupported-step-type", |args| {
            args.set("step_type", format!("{:?}", self.0));
        });
        f.write_str(&message)
    }
}

impl std::error::Error for UnsupportedStepType {}

impl core::convert::TryFrom<StepType> for StepKeyword {
    type Error = UnsupportedStepType;

    fn try_from(ty: StepType) -> Result<Self, Self::Error> {
        match ty {
            StepType::Given => Ok(Self::Given),
            StepType::When => Ok(Self::When),
            StepType::Then => Ok(Self::Then),
            #[expect(unreachable_patterns, reason = "guard future StepType variants")]
            other => match format!("{other:?}") {
                s if s == "And" => Ok(Self::And),
                s if s == "But" => Ok(Self::But),
                _ => Err(UnsupportedStepType(other)),
            },
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use gherkin::StepType;
    use rstest::rstest;
    use std::str::FromStr;

    #[expect(clippy::expect_used, reason = "test helper with descriptive failures")]
    fn parse_kw(input: &str) -> StepKeyword {
        StepKeyword::from_str(input).expect("valid step keyword")
    }

    #[expect(clippy::expect_used, reason = "test helper with descriptive failures")]
    fn kw_from_type(ty: StepType) -> StepKeyword {
        StepKeyword::try_from(ty).expect("valid step type")
    }

    #[rstest]
    #[case("Given", StepKeyword::Given)]
    #[case("given", StepKeyword::Given)]
    #[case("\tThEn\n", StepKeyword::Then)]
    #[case("AND", StepKeyword::And)]
    #[case(" but ", StepKeyword::But)]
    fn parses_case_insensitively(#[case] input: &str, #[case] expected: StepKeyword) {
        assert!(matches!(StepKeyword::from_str(input), Ok(val) if val == expected));
        assert_eq!(parse_kw(input), expected);
    }

    #[rstest]
    #[case(StepType::Given, StepKeyword::Given)]
    #[case(StepType::When, StepKeyword::When)]
    #[case(StepType::Then, StepKeyword::Then)]
    fn maps_step_type(#[case] input: StepType, #[case] expected: StepKeyword) {
        assert_eq!(kw_from_type(input), expected);
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
                let detail = localisation::message_with_args("placeholder-syntax-suffix", |args| {
                    args.set("placeholder", name.clone());
                });
                format!(" {detail}")
            })
            .unwrap_or_default();
        localisation::message_with_args("placeholder-syntax-detail", |args| {
            args.set("reason", self.message.clone());
            args.set("position", self.position.to_string());
            args.set("suffix", suffix);
        })
    }
}

impl fmt::Display for PlaceholderSyntaxError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let message = localisation::message_with_args("placeholder-syntax", |args| {
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
                    localisation::message_with_args("step-pattern-not-compiled", |args| {
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
            Self::PatternMismatch => localisation::message("placeholder-pattern-mismatch"),
            Self::InvalidPlaceholder(details) => {
                localisation::message_with_args("placeholder-invalid-placeholder", |args| {
                    args.set("details", details.clone());
                })
            }
            Self::InvalidPattern(pattern) => {
                localisation::message_with_args("placeholder-invalid-pattern", |args| {
                    args.set("pattern", pattern.clone());
                })
            }
            Self::NotCompiled { pattern } => {
                localisation::message_with_args("placeholder-not-compiled", |args| {
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

/// Type alias for the stored step function pointer.
pub type StepFn = for<'a> fn(
    &crate::context::StepContext<'a>,
    &str,
    Option<&str>,
    Option<&[&[&str]]>,
) -> Result<Option<Box<dyn std::any::Any>>, crate::StepError>;
