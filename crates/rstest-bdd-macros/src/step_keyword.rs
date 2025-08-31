//! Local representation of step keywords used during macro expansion.
//!
//! This lightweight enum mirrors the variants provided by `rstest-bdd` but
//! avoids a compile-time dependency on that crate. It is only used internally
//! for parsing feature files and generating code. While the enum includes `And`
//! and `But` for completeness, feature parsing normalizes them to the preceding
//! primary keyword.

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

/// Error produced when encountering an unsupported `StepType`.
#[derive(Debug, thiserror::Error)]
#[error("unsupported step type: {0:?}")]
pub(crate) struct UnsupportedStepType(pub StepType);

impl From<&str> for StepKeyword {
    fn from(value: &str) -> Self {
        let s = value.trim();
        if s.eq_ignore_ascii_case("given") {
            Self::Given
        } else if s.eq_ignore_ascii_case("when") {
            Self::When
        } else if s.eq_ignore_ascii_case("then") {
            Self::Then
        } else if s.eq_ignore_ascii_case("and") {
            Self::And
        } else if s.eq_ignore_ascii_case("but") {
            Self::But
        } else {
            // Use the original, untrimmed `value` for clearer diagnostics.
            panic!("invalid step keyword: {value}")
        }
    }
}

impl TryFrom<StepType> for StepKeyword {
    type Error = UnsupportedStepType;

    fn try_from(ty: StepType) -> Result<Self, Self::Error> {
        let kw = match ty {
            StepType::Given => Self::Given,
            StepType::When => Self::When,
            StepType::Then => Self::Then,
        };
        Ok(kw)
    }
}

impl TryFrom<&Step> for StepKeyword {
    type Error = UnsupportedStepType;

    fn try_from(step: &Step) -> Result<Self, Self::Error> {
        match step.keyword.trim() {
            s if s.eq_ignore_ascii_case("and") => Ok(Self::And),
            s if s.eq_ignore_ascii_case("but") => Ok(Self::But),
            _ => Self::try_from(step.ty),
        }
    }
}

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
    /// Resolve conjunctions to the semantic keyword of the previous step.
    ///
    /// Leading conjunctions default to `Given` to maintain a sensible
    /// baseline when no prior step exists.
    pub(crate) fn resolve(self, prev: &mut Option<Self>) -> Self {
        if matches!(self, Self::And | Self::But) {
            prev.unwrap_or(Self::Given)
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
        assert_eq!(StepKeyword::from(input), expected);
    }
}
