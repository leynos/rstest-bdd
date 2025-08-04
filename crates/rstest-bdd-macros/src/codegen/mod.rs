//! Code generation utilities for the proc macros.

use proc_macro2::TokenStream as TokenStream2;
use quote::quote;

pub(crate) mod scenario;
pub(crate) mod wrapper;

/// Convert a [`StepKeyword`] into a quoted token.
pub(crate) fn keyword_to_token(keyword: rstest_bdd::StepKeyword) -> TokenStream2 {
    match keyword {
        rstest_bdd::StepKeyword::Given => quote! { rstest_bdd::StepKeyword::Given },
        rstest_bdd::StepKeyword::When => quote! { rstest_bdd::StepKeyword::When },
        rstest_bdd::StepKeyword::Then => quote! { rstest_bdd::StepKeyword::Then },
        rstest_bdd::StepKeyword::And => quote! { rstest_bdd::StepKeyword::And },
        rstest_bdd::StepKeyword::But => quote! { rstest_bdd::StepKeyword::But },
    }
}
