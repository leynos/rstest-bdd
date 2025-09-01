//! Local representation of step keywords used during macro expansion.
//!
//! This lightweight enum mirrors the variants provided by `rstest-bdd` but
//! avoids a compile-time dependency on that crate. It is only used internally
//! for parsing feature files and generating code. While the enum includes `And`
//! and `But` for completeness. Conjunction resolution happens during code
//! generation via `StepKeyword::resolve`, typically seeded to the first primary
//! keyword; when unseeded it falls back to `Given`.

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

impl From<&str> for StepKeyword {
    fn from(value: &str) -> Self {
        value
            .parse()
            .unwrap_or_else(|_| panic!("invalid step keyword: {value}"))
    }
}

impl From<StepType> for StepKeyword {
    fn from(ty: StepType) -> Self {
        match ty {
            StepType::Given => Self::Given,
            StepType::When => Self::When,
            StepType::Then => Self::Then,
            #[expect(unreachable_patterns, reason = "guard future variants")]
            _ => panic!("unsupported step type: {ty:?}"),
        }
    }
}

/// Note: conjunction detection via `step.keyword` is English-only
/// ("And"/"But"). Other languages rely on `StepType`; textual
/// conjunctions are not preserved.
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
        assert_eq!(StepKeyword::from(input), expected);
    }

    #[test]
    fn rejects_invalid_keyword_via_from_str() {
        assert!("invalid".parse::<StepKeyword>().is_err());
    }
}
