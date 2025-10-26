//! Local representation of step keywords used during macro expansion.
//!
//! This lightweight enum mirrors the variants provided by `rstest-bdd` but
//! avoids a compile-time dependency on that crate. It is only used internally
//! for parsing feature files and generating code. The enum includes `And` and
//! `But` for completeness; conjunction resolution is centralized in
//! `validation::steps::resolve_keywords` and consumed by validation and code generation,
//! falling back to `Given` when unseeded.

use gherkin::{Step, StepType};
use proc_macro2::TokenStream as TokenStream2;
use quote::{quote, ToTokens};
use std::fmt;

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct StepKeywordParseError(pub String);

impl fmt::Display for StepKeywordParseError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "invalid step keyword: {}", self.0)
    }
}

impl std::error::Error for StepKeywordParseError {}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) struct UnsupportedStepType(pub StepType);

impl fmt::Display for UnsupportedStepType {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "unsupported step type: {:?}", self.0)
    }
}

impl std::error::Error for UnsupportedStepType {}

/// Keyword used to categorize a step definition.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub(crate) enum StepKeyword {
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

/// Trim and match step keywords case-insensitively, returning `None` when no
/// known keyword is found.
///
/// Note: textual conjunction detection handles English ("And"/"But");
/// other locales rely on `StepType` and are resolved centrally.
fn parse_step_keyword(value: &str) -> Option<StepKeyword> {
    let s = value.trim();
    if s.eq_ignore_ascii_case("given") {
        Some(StepKeyword::Given)
    } else if s.eq_ignore_ascii_case("when") {
        Some(StepKeyword::When)
    } else if s.eq_ignore_ascii_case("then") {
        Some(StepKeyword::Then)
    } else if s.eq_ignore_ascii_case("and") {
        Some(StepKeyword::And)
    } else if s.eq_ignore_ascii_case("but") {
        Some(StepKeyword::But)
    } else {
        None
    }
}

impl core::str::FromStr for StepKeyword {
    type Err = StepKeywordParseError;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        parse_step_keyword(s).ok_or_else(|| StepKeywordParseError(s.trim().to_string()))
    }
}

impl core::convert::TryFrom<&str> for StepKeyword {
    type Error = StepKeywordParseError;

    fn try_from(value: &str) -> Result<Self, Self::Error> {
        value.parse()
    }
}

impl core::convert::TryFrom<StepType> for StepKeyword {
    type Error = UnsupportedStepType;

    fn try_from(ty: StepType) -> Result<Self, Self::Error> {
        match ty {
            StepType::Given => Ok(Self::Given),
            StepType::When => Ok(Self::When),
            StepType::Then => Ok(Self::Then),
            // Intentionally expect `unreachable_patterns` for the current StepType set.
            // New variants break the expectation and fail the build.
            #[expect(unreachable_patterns, reason = "guard future StepType variants")]
            other => match format!("{other:?}") {
                s if s == "And" => Ok(Self::And),
                s if s == "But" => Ok(Self::But),
                _ => Err(UnsupportedStepType(other)),
            },
        }
    }
}

impl core::convert::TryFrom<&Step> for StepKeyword {
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
        let variant = match self {
            Self::Given => quote!(Given),
            Self::When => quote!(When),
            Self::Then => quote!(Then),
            Self::And => quote!(And),
            Self::But => quote!(But),
        };
        tokens.extend(quote! { #path::StepKeyword::#variant });
    }
}

impl StepKeyword {
    /// Resolve conjunctions to the semantic keyword of the previous step or a
    /// seeded first primary keyword.
    ///
    /// `resolve_keywords` seeds `prev` with the first primary keyword so leading
    /// conjunctions inherit that seed. When `prev` is `None`, conjunctions
    /// default to `Given`.
    pub(crate) fn resolve(self, prev: &mut Option<Self>) -> Self {
        if matches!(self, Self::And | Self::But) {
            prev.as_ref().copied().unwrap_or(Self::Given)
        } else {
            *prev = Some(self);
            self
        }
    }

    /// Canonical display label used by diagnostics and generated code.
    #[cfg(feature = "compile-time-validation")]
    pub(crate) fn display_name(self) -> &'static str {
        match self {
            Self::Given => "Given",
            Self::When => "When",
            Self::Then => "Then",
            Self::And => "And",
            Self::But => "But",
        }
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
    #[rstest::rstest]
    #[case(StepType::Given, StepKeyword::Given)]
    #[case(StepType::When, StepKeyword::When)]
    #[case(StepType::Then, StepKeyword::Then)]
    fn maps_step_type(#[case] ty: StepType, #[case] expected: StepKeyword) {
        assert_eq!(StepKeyword::try_from(ty).ok(), Some(expected));
    }
}
