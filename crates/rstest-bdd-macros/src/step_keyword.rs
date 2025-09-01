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

// Trim and match step keywords case-insensitively, returning `None` when no
// known keyword is found.
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

impl From<&str> for StepKeyword {
    fn from(value: &str) -> Self {
        parse_step_keyword(value).unwrap_or_else(|| panic!("invalid step keyword: {value}"))
    }
}

impl From<StepType> for StepKeyword {
    fn from(ty: StepType) -> Self {
        if ty == StepType::Given {
            Self::Given
        } else if ty == StepType::When {
            Self::When
        } else {
            Self::Then
        }
    }
}

impl From<&Step> for StepKeyword {
    fn from(step: &Step) -> Self {
        match step.keyword.trim() {
            s if s.eq_ignore_ascii_case("and") => Self::And,
            s if s.eq_ignore_ascii_case("but") => Self::But,
            _ => Self::from(step.ty),
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
    /// Resolve conjunctions to the semantic keyword of the previous step or a
    /// seeded first primary keyword.
    ///
    /// `process_steps` seeds `prev` with the first primary keyword so leading
    /// conjunctions inherit that seed. When `prev` is `None`, conjunctions
    /// default to `Given`.
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

    #[test]
    #[should_panic(expected = "invalid step keyword: invalid")]
    fn panics_on_invalid_keyword() {
        let _ = StepKeyword::from("invalid");
    }
}
