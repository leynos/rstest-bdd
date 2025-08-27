//! Local representation of step keywords used during macro expansion.
//!
//! This lightweight enum mirrors the variants provided by `rstest-bdd` but
//! avoids a compile-time dependency on that crate. It is only used internally
//! for parsing feature files and generating code. While the enum includes `And`
//! and `But` for completeness, feature parsing normalises them to the preceding
//! primary keyword.

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

impl From<&str> for StepKeyword {
    fn from(value: &str) -> Self {
        match value.trim().to_ascii_lowercase().as_str() {
            "given" => Self::Given,
            "when" => Self::When,
            "then" => Self::Then,
            "and" => Self::And,
            "but" => Self::But,
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

#[cfg(test)]
mod tests {
    use super::*;
    use rstest::rstest;

    #[rstest]
    #[case("Given", StepKeyword::Given)]
    #[case("given", StepKeyword::Given)]
    #[case(" WhEn ", StepKeyword::When)]
    fn parses_case_insensitively(#[case] input: &str, #[case] expected: StepKeyword) {
        assert_eq!(StepKeyword::from(input), expected);
    }
}
