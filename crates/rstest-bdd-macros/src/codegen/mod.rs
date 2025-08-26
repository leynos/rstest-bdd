//! Code generation utilities for the proc macros.

use proc_macro2::TokenStream as TokenStream2;
use quote::quote;

pub(crate) mod scenario;
pub(crate) mod wrapper;

/// Convert a [`StepKeyword`] into a quoted token.
pub(crate) fn keyword_to_token(keyword: crate::StepKeyword) -> TokenStream2 {
    match keyword {
        crate::StepKeyword::Given => quote! { ::rstest_bdd::StepKeyword::Given },
        crate::StepKeyword::When => quote! { ::rstest_bdd::StepKeyword::When },
        crate::StepKeyword::Then => quote! { ::rstest_bdd::StepKeyword::Then },
        crate::StepKeyword::And => quote! { ::rstest_bdd::StepKeyword::And },
        crate::StepKeyword::But => quote! { ::rstest_bdd::StepKeyword::But },
    }
}
