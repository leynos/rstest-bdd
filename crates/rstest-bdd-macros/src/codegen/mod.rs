//! Code generation utilities for the proc macros.
//!
//! This module emits fully-qualified paths (`::rstest_bdd::…`) so the macros crate
//! does not depend on the runtime crate at compile-time.

use proc_macro_crate::{FoundCrate, crate_name};
use proc_macro2::{Ident, Span, TokenStream as TokenStream2};
use quote::quote;

pub(crate) mod scenario;
pub(crate) mod wrapper;

/// Return a token stream pointing to the `rstest_bdd` crate or its renamed form.
pub(crate) fn rstest_bdd_path() -> TokenStream2 {
    let found = crate_name("rstest-bdd").unwrap_or_else(|e| {
        // The runtime crate must be present for generated code; a missing entry
        // indicates a misconfigured build rather than a recoverable error.
        panic!("rstest-bdd crate not found: {e}");
    });
    let ident = match found {
        FoundCrate::Itself => Ident::new("rstest_bdd", Span::call_site()),
        FoundCrate::Name(name) => Ident::new(&name, Span::call_site()),
    };
    quote! { ::#ident }
}
