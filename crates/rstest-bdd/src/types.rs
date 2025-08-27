//! Core types and error enums used across the crate.
//! This module defines the lightweight wrappers for pattern and step text,
//! the step keyword enum with parsing helpers, error types, and common type
//! aliases used by the registry and runner.

use gherkin::StepType;
use proc_macro2::TokenStream;
use quote::ToTokens;
use std::fmt;
use std::fmt::Write as _;
use std::str::FromStr;
use thiserror::Error;

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

/// Keyword used to categorise a step definition.
///
/// `And` and `But` are parsed distinctly but scenario processing resolves them
/// to the preceding primary keyword before step lookup.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum StepKeyword {
    /// Setup preconditions for a scenario.
    Given,
    /// Perform an action when testing behaviour.
    When,
    /// Assert the expected outcome of a scenario.
    Then,
    /// Additional conditions that share context with the previous step.
    ///
    /// During scenario processing this keyword resolves to the preceding
    /// primary keyword.
    And,
    /// Negative or contrasting conditions.
    ///
    /// Resolves to the preceding primary keyword like [`Self::And`].
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
#[derive(Debug, Error)]
#[error("invalid step keyword: {0}")]
pub struct StepKeywordParseError(pub String);

impl FromStr for StepKeyword {
    type Err = StepKeywordParseError;

    fn from_str(value: &str) -> Result<Self, Self::Err> {
        let kw = match value {
            "Given" => Self::Given,
            "When" => Self::When,
            "Then" => Self::Then,
            "And" => Self::And,
            "But" => Self::But,
            other => return Err(StepKeywordParseError(other.to_string())),
        };
        Ok(kw)
    }
}

impl From<&str> for StepKeyword {
    fn from(value: &str) -> Self {
        Self::from_str(value).unwrap_or_else(|_| panic!("invalid step keyword: {value}"))
    }
}

impl From<StepType> for StepKeyword {
    fn from(ty: StepType) -> Self {
        match ty {
            StepType::Given => Self::Given,
            StepType::When => Self::When,
            StepType::Then => Self::Then,
            #[expect(
                unreachable_patterns,
                reason = "future-proof against upstream StepType variants"
            )]
            _ => unreachable!("unsupported step type: {ty:?}"),
        }
    }
}

impl ToTokens for StepKeyword {
    fn to_tokens(&self, tokens: &mut TokenStream) {
        let t = match self {
            Self::Given => quote::quote! { rstest_bdd::StepKeyword::Given },
            Self::When => quote::quote! { rstest_bdd::StepKeyword::When },
            Self::Then => quote::quote! { rstest_bdd::StepKeyword::Then },
            Self::And => quote::quote! { rstest_bdd::StepKeyword::And },
            Self::But => quote::quote! { rstest_bdd::StepKeyword::But },
        };
        tokens.extend([t]);
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
        let mut msg = format!("{} at byte {} (zero-based)", self.message, self.position);
        if let Some(name) = &self.placeholder {
            let _ = write!(msg, " for placeholder `{name}`");
        }
        msg
    }
}

impl fmt::Display for PlaceholderSyntaxError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "invalid placeholder syntax: {}", self.user_message())
    }
}

impl std::error::Error for PlaceholderSyntaxError {}

/// Errors that may occur when compiling a [`StepPattern`].
#[derive(Debug, Error)]
#[non_exhaustive]
pub enum StepPatternError {
    /// Placeholder syntax in the pattern is invalid.
    #[error(transparent)]
    PlaceholderSyntax(#[from] PlaceholderSyntaxError),
    /// The generated regular expression failed to compile.
    #[error("{0}")]
    InvalidPattern(#[from] regex::Error),
}

/// Error conditions that may arise when extracting placeholders.
#[derive(Debug, Error)]
#[non_exhaustive]
pub enum PlaceholderError {
    /// The supplied text did not match the step pattern.
    #[error("pattern mismatch")]
    PatternMismatch,
    /// The step pattern contained invalid placeholder syntax.
    #[error("invalid placeholder syntax: {0}")]
    InvalidPlaceholder(String),
    /// The step pattern could not be compiled into a regular expression.
    #[error("invalid step pattern: {0}")]
    InvalidPattern(String),
}

impl From<StepPatternError> for PlaceholderError {
    fn from(e: StepPatternError) -> Self {
        match e {
            StepPatternError::PlaceholderSyntax(err) => {
                Self::InvalidPlaceholder(err.user_message())
            }
            StepPatternError::InvalidPattern(re) => Self::InvalidPattern(re.to_string()),
        }
    }
}

/// Type alias for the stored step function pointer.
pub type StepFn = for<'a> fn(
    &crate::context::StepContext<'a>,
    &str,
    Option<&str>,
    Option<&[&[&str]]>,
) -> Result<(), crate::StepError>;
