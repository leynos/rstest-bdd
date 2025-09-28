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
    match crate_name("rstest-bdd") {
        Ok(found) => {
            let ident = match found {
                FoundCrate::Itself => Ident::new("rstest_bdd", Span::call_site()),
                FoundCrate::Name(name) => Ident::new(&name, Span::call_site()),
            };
            quote! { ::#ident }
        }
        Err(e) => {
            if cfg!(test) {
                return quote! { ::rstest_bdd };
            }
            panic!("rstest-bdd crate not found: {e}");
        }
    }
}
