//! Step keyword support for macro expansion.
//!
//! This module wraps the shared [`rstest_bdd_patterns::StepKeyword`] type and
//! provides additional macro-specific implementations for code generation.
//! A newtype wrapper is used to satisfy Rust's orphan rules when implementing
//! foreign traits like `ToTokens`.

use gherkin::Step;
use proc_macro2::TokenStream as TokenStream2;
use quote::{ToTokens, quote};
use rstest_bdd_patterns::StepKeyword as BaseKeyword;
use std::fmt;
use std::str::FromStr;

// Re-export error types from the patterns crate.
pub(crate) use rstest_bdd_patterns::UnsupportedStepType;

/// Keyword used to categorise a step definition.
///
/// This is a newtype wrapper around [`rstest_bdd_patterns::StepKeyword`] that
/// enables implementing `ToTokens` and other macro-specific traits while
/// satisfying Rust's orphan rules.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub(crate) struct StepKeyword(BaseKeyword);

// Associated constants with PascalCase names to mirror enum variant syntax.
// The lint suppression is intentional to preserve API ergonomics.
#[expect(
    non_upper_case_globals,
    reason = "associated constants mirror enum variant naming for API consistency"
)]
impl StepKeyword {
    /// Setup preconditions for a scenario.
    pub(crate) const Given: Self = Self(BaseKeyword::Given);
    /// Perform an action when testing behaviour.
    pub(crate) const When: Self = Self(BaseKeyword::When);
    /// Assert the expected outcome of a scenario.
    pub(crate) const Then: Self = Self(BaseKeyword::Then);
    /// Additional conditions that share context with the previous step.
    pub(crate) const And: Self = Self(BaseKeyword::And);
    /// Negative or contrasting conditions.
    pub(crate) const But: Self = Self(BaseKeyword::But);

    /// Return the keyword as a string slice.
    #[must_use]
    pub(crate) fn as_str(self) -> &'static str { self.0.as_str() }

    /// Resolve conjunctions to the semantic keyword of the previous step.
    ///
    /// When the current keyword is `And` or `But`, returns the value stored in
    /// `prev`. For primary keywords (`Given`/`When`/`Then`), updates `prev` and
    /// returns the keyword unchanged.
    #[must_use]
    pub(crate) fn resolve(self, prev: &mut Option<Self>) -> Self {
        // Convert to/from the patterns type for the resolution logic
        let mut inner_prev = prev.map(|p| p.0);
        let resolved = self.0.resolve(&mut inner_prev);
        *prev = inner_prev.map(Self);
        Self(resolved)
    }
}

/// Error returned when parsing a [`StepKeyword`] from a string fails.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct StepKeywordParseError(pub String);

impl fmt::Display for StepKeywordParseError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "invalid step keyword: {}", self.0)
    }
}

impl std::error::Error for StepKeywordParseError {}

impl FromStr for StepKeyword {
    type Err = StepKeywordParseError;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        rstest_bdd_patterns::StepKeyword::from_str(s)
            .map(Self)
            .map_err(|e| StepKeywordParseError(e.0))
    }
}

impl TryFrom<&str> for StepKeyword {
    type Error = StepKeywordParseError;

    fn try_from(value: &str) -> Result<Self, Self::Error> { value.parse() }
}

impl TryFrom<gherkin::StepType> for StepKeyword {
    type Error = UnsupportedStepType;

    fn try_from(ty: gherkin::StepType) -> Result<Self, Self::Error> {
        rstest_bdd_patterns::StepKeyword::try_from(ty).map(Self)
    }
}

impl TryFrom<&Step> for StepKeyword {
    type Error = UnsupportedStepType;

    fn try_from(step: &Step) -> Result<Self, Self::Error> {
        match Self::try_from(step.ty) {
            Ok(primary @ (Self::Given | Self::When | Self::Then)) => match step.keyword.trim() {
                s if s.eq_ignore_ascii_case("and") => Ok(Self::And),
                s if s.eq_ignore_ascii_case("but") => Ok(Self::But),
                _ => Ok(primary),
            },
            Ok(k) => Ok(k),
            Err(_) => match step.keyword.trim() {
                s if s.eq_ignore_ascii_case("and") => Ok(Self::And),
                s if s.eq_ignore_ascii_case("but") => Ok(Self::But),
                _ => Err(UnsupportedStepType(step.ty)),
            },
        }
    }
}

/// Textual conjunction detection handles English ("And"/"But").
/// Non-English conjunctions rely on `gherkin::StepType` and are resolved centrally.
impl ToTokens for StepKeyword {
    fn to_tokens(&self, tokens: &mut TokenStream2) {
        let path = crate::codegen::rstest_bdd_path();
        let variant = match self.0 {
            BaseKeyword::Given => quote!(Given),
            BaseKeyword::When => quote!(When),
            BaseKeyword::Then => quote!(Then),
            BaseKeyword::And => quote!(And),
            BaseKeyword::But => quote!(But),
        };
        tokens.extend(quote! { #path::StepKeyword::#variant });
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use gherkin::StepType;
    use rstest::rstest;

    #[expect(clippy::expect_used, reason = "test helper with descriptive failures")]
    fn parse_kw(input: &str) -> StepKeyword {
        StepKeyword::try_from(input).expect("valid step keyword")
    }

    #[rstest]
    #[case("Given", StepKeyword::Given)]
    #[case("given", StepKeyword::Given)]
    #[case(" WhEn ", StepKeyword::When)]
    #[case("AND", StepKeyword::And)]
    #[case(" but ", StepKeyword::But)]
    fn parses_case_insensitively(#[case] input: &str, #[case] expected: StepKeyword) {
        assert_eq!(parse_kw(input), expected);
    }

    #[test]
    fn rejects_invalid_keyword_via_from_str() {
        assert!("invalid".parse::<StepKeyword>().is_err());
    }

    #[rstest]
    #[case(StepType::Given, StepKeyword::Given)]
    #[case(StepType::When, StepKeyword::When)]
    #[case(StepType::Then, StepKeyword::Then)]
    fn maps_step_type(#[case] ty: StepType, #[case] expected: StepKeyword) {
        assert_eq!(StepKeyword::try_from(ty).ok(), Some(expected));
    }

    #[test]
    fn resolve_returns_previous_for_conjunctions() {
        let mut prev = Some(StepKeyword::When);
        assert_eq!(StepKeyword::And.resolve(&mut prev), StepKeyword::When);
        assert_eq!(StepKeyword::But.resolve(&mut prev), StepKeyword::When);
        assert_eq!(prev, Some(StepKeyword::When));
    }

    #[test]
    fn resolve_updates_previous_for_primary_keywords() {
        let mut prev = Some(StepKeyword::Given);
        assert_eq!(StepKeyword::When.resolve(&mut prev), StepKeyword::When);
        assert_eq!(prev, Some(StepKeyword::When));
    }
}
