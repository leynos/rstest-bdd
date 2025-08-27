//! Local representation of step keywords used during macro expansion.
//!
//! This lightweight enum mirrors the variants provided by `rstest-bdd` but
//! avoids a compile-time dependency on that crate. It is only used internally
//! for parsing feature files and generating code. While the enum includes `And`
//! and `But` for completeness, feature parsing normalises them to the preceding
//! primary keyword.

use gherkin::StepType;
use proc_macro2::TokenStream as TokenStream2;
use quote::{ToTokens, quote};

/// Keyword used to categorise a step definition.
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

impl From<StepType> for StepKeyword {
    fn from(value: StepType) -> Self {
        #[expect(unreachable_patterns, reason = "panic on future StepType variants")]
        match value {
            StepType::Given => Self::Given,
            StepType::When => Self::When,
            StepType::Then => Self::Then,
            #[cfg(any())]
            StepType::And => Self::And,
            #[cfg(any())]
            StepType::But => Self::But,
            _ => panic!("unsupported step type: {value:?}"),
        }
    }
}

impl From<&str> for StepKeyword {
    fn from(value: &str) -> Self {
        match value.trim() {
            "Given" => Self::Given,
            "When" => Self::When,
            "Then" => Self::Then,
            "And" => Self::And,
            "But" => Self::But,
            other => panic!("invalid step keyword: {other}"),
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
