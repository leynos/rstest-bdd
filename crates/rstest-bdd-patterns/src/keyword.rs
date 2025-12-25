//! Shared step keyword type and parsing utilities.
//!
//! This module provides the canonical [`StepKeyword`] enum used by both the
//! runtime and proc-macro crates, ensuring consistent keyword handling across
//! compile-time validation and runtime execution.

use gherkin::StepType;
use std::fmt;
use std::str::FromStr;

/// Keyword used to categorise a step definition.
///
/// The enum includes `And` and `But` variants for completeness, but feature
/// parsing resolves them against the preceding `Given`/`When`/`Then` using
/// the [`resolve`](Self::resolve) method.
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
    ///
    /// # Examples
    ///
    /// ```
    /// use rstest_bdd_patterns::StepKeyword;
    ///
    /// assert_eq!(StepKeyword::Given.as_str(), "Given");
    /// assert_eq!(StepKeyword::And.as_str(), "And");
    /// ```
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

    /// Resolve conjunctions to the semantic keyword of the previous step.
    ///
    /// When the current keyword is `And` or `But`, returns the value stored in
    /// `prev`. For primary keywords (`Given`/`When`/`Then`), updates `prev` and
    /// returns the keyword unchanged.
    ///
    /// Callers typically seed `prev` with the first primary keyword in a
    /// sequence, defaulting to `Given` when none is found.
    ///
    /// # Examples
    ///
    /// ```
    /// use rstest_bdd_patterns::StepKeyword;
    ///
    /// let mut prev = Some(StepKeyword::Given);
    /// assert_eq!(StepKeyword::And.resolve(&mut prev), StepKeyword::Given);
    /// assert_eq!(StepKeyword::When.resolve(&mut prev), StepKeyword::When);
    /// assert_eq!(prev, Some(StepKeyword::When));
    /// ```
    #[must_use]
    pub fn resolve(self, prev: &mut Option<Self>) -> Self {
        if matches!(self, Self::And | Self::But) {
            prev.as_ref().copied().unwrap_or(Self::Given)
        } else {
            *prev = Some(self);
            self
        }
    }
}

/// Error returned when parsing a [`StepKeyword`] from a string fails.
///
/// Contains the unrecognised keyword text for diagnostic purposes.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct StepKeywordParseError(pub String);

impl fmt::Display for StepKeywordParseError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "invalid step keyword: {}", self.0)
    }
}

impl std::error::Error for StepKeywordParseError {}

impl FromStr for StepKeyword {
    type Err = StepKeywordParseError;

    fn from_str(value: &str) -> Result<Self, Self::Err> {
        let trimmed = value.trim();
        if trimmed.eq_ignore_ascii_case("given") {
            Ok(Self::Given)
        } else if trimmed.eq_ignore_ascii_case("when") {
            Ok(Self::When)
        } else if trimmed.eq_ignore_ascii_case("then") {
            Ok(Self::Then)
        } else if trimmed.eq_ignore_ascii_case("and") {
            Ok(Self::And)
        } else if trimmed.eq_ignore_ascii_case("but") {
            Ok(Self::But)
        } else {
            Err(StepKeywordParseError(trimmed.to_string()))
        }
    }
}

impl TryFrom<&str> for StepKeyword {
    type Error = StepKeywordParseError;

    fn try_from(value: &str) -> Result<Self, Self::Error> {
        value.parse()
    }
}

/// Error raised when converting a parsed Gherkin [`StepType`] into a
/// [`StepKeyword`] fails.
///
/// Captures the offending [`StepType`] to help callers diagnose missing
/// language support.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct UnsupportedStepType(pub StepType);

impl fmt::Display for UnsupportedStepType {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "unsupported step type: {:?}", self.0)
    }
}

impl std::error::Error for UnsupportedStepType {}

impl TryFrom<StepType> for StepKeyword {
    type Error = UnsupportedStepType;

    fn try_from(ty: StepType) -> Result<Self, Self::Error> {
        match ty {
            StepType::Given => Ok(Self::Given),
            StepType::When => Ok(Self::When),
            StepType::Then => Ok(Self::Then),
            // Guard future StepType variants; new variants break the expectation
            // and fail the build.
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
    use rstest::rstest;

    #[expect(clippy::expect_used, reason = "test helper with descriptive failures")]
    fn parse_kw(input: &str) -> StepKeyword {
        input
            .parse()
            .expect("test input should parse to a valid keyword")
    }

    #[rstest]
    #[case("Given", StepKeyword::Given)]
    #[case("given", StepKeyword::Given)]
    #[case(" WhEn ", StepKeyword::When)]
    #[case("THEN", StepKeyword::Then)]
    #[case("AND", StepKeyword::And)]
    #[case(" but ", StepKeyword::But)]
    fn parses_case_insensitively(#[case] input: &str, #[case] expected: StepKeyword) {
        assert_eq!(parse_kw(input), expected);
    }

    #[test]
    #[expect(clippy::expect_used, reason = "test verifies error case with descriptive failure")]
    fn rejects_invalid_keyword() {
        let result = "invalid".parse::<StepKeyword>();
        assert!(result.is_err());
        let err = result.expect_err("expected parse error for invalid keyword");
        assert_eq!(err.0, "invalid");
    }

    #[expect(clippy::expect_used, reason = "test helper with descriptive failures")]
    fn kw_from_type(ty: StepType) -> StepKeyword {
        StepKeyword::try_from(ty).expect("test StepType should convert to StepKeyword")
    }

    #[rstest]
    #[case(StepType::Given, StepKeyword::Given)]
    #[case(StepType::When, StepKeyword::When)]
    #[case(StepType::Then, StepKeyword::Then)]
    fn maps_step_type(#[case] ty: StepType, #[case] expected: StepKeyword) {
        assert_eq!(kw_from_type(ty), expected);
    }

    #[test]
    fn as_str_returns_canonical_name() {
        assert_eq!(StepKeyword::Given.as_str(), "Given");
        assert_eq!(StepKeyword::When.as_str(), "When");
        assert_eq!(StepKeyword::Then.as_str(), "Then");
        assert_eq!(StepKeyword::And.as_str(), "And");
        assert_eq!(StepKeyword::But.as_str(), "But");
    }

    #[test]
    fn resolve_returns_previous_for_conjunctions() {
        let mut prev = Some(StepKeyword::When);
        assert_eq!(StepKeyword::And.resolve(&mut prev), StepKeyword::When);
        assert_eq!(StepKeyword::But.resolve(&mut prev), StepKeyword::When);
        // prev unchanged for conjunctions
        assert_eq!(prev, Some(StepKeyword::When));
    }

    #[test]
    fn resolve_updates_previous_for_primary_keywords() {
        let mut prev = Some(StepKeyword::Given);
        assert_eq!(StepKeyword::When.resolve(&mut prev), StepKeyword::When);
        assert_eq!(prev, Some(StepKeyword::When));
        assert_eq!(StepKeyword::Then.resolve(&mut prev), StepKeyword::Then);
        assert_eq!(prev, Some(StepKeyword::Then));
    }

    #[test]
    fn resolve_defaults_to_given_when_unseeded() {
        let mut prev = None;
        assert_eq!(StepKeyword::And.resolve(&mut prev), StepKeyword::Given);
        assert_eq!(prev, None); // conjunctions don't update prev
    }
}
