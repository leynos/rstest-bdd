//! Local representation of step keywords used during macro expansion.
//!
//! This lightweight enum mirrors the variants provided by `rstest-bdd` but
//! avoids a compile-time dependency on that crate. It is only used internally
//! for parsing feature files and generating code. The enum includes `And` and
//! `But` for completeness; conjunction resolution is centralised in
//! `validation::steps::resolve_keywords` and consumed by code generation,
//! falling back to `Given` when unseeded.

use gherkin::{Step, StepType};
use proc_macro2::TokenStream as TokenStream2;
use quote::{ToTokens, quote};

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
/// Note: textual conjunction detection is English-only ("And"/"But"); other
/// locales are handled via `StepType`.
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
    type Err = &'static str;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        parse_step_keyword(s).ok_or("invalid step keyword")
    }
}

#[derive(Debug)]
pub(crate) struct UnsupportedStepType(pub StepType);

impl core::fmt::Display for UnsupportedStepType {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        write!(f, "unsupported step type: {:?}", self.0)
    }
}

impl core::convert::TryFrom<&str> for StepKeyword {
    type Error = &'static str;

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
            #[expect(unreachable_patterns, reason = "guard future StepType variants")]
            other => Err(UnsupportedStepType(other)),
        }
    }
}

impl core::convert::TryFrom<&Step> for StepKeyword {
    type Error = UnsupportedStepType;

    fn try_from(step: &Step) -> Result<Self, Self::Error> {
        match step.keyword.trim() {
            s if s.eq_ignore_ascii_case("and") => Ok(Self::And),
            s if s.eq_ignore_ascii_case("but") => Ok(Self::But),
            _ => Self::try_from(step.ty),
        }
    }
}

/// Textual conjunction detection is English-only ("And"/"But"); other
/// languages are handled via `gherkin::StepType` provided by the parser.
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
    /// `process_steps` seeds `prev` with the first primary keyword so leading
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
}

#[cfg(test)]
mod tests {
    use super::*;
    use rstest::rstest;

    #[rstest]
    #[case("Given", StepKeyword::Given)]
    #[case("given", StepKeyword::Given)]
    #[case(" WhEn ", StepKeyword::When)]
    #[case("AND", StepKeyword::And)]
    #[case(" but ", StepKeyword::But)]
    fn parses_case_insensitively(#[case] input: &str, #[case] expected: StepKeyword) {
        assert_eq!(
            StepKeyword::try_from(input).unwrap_or_else(|e| panic!("valid step keyword: {e}")),
            expected
        );
    }

    #[test]
    fn rejects_invalid_keyword_via_from_str() {
        assert!("invalid".parse::<StepKeyword>().is_err());
    }
}
