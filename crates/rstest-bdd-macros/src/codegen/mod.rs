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
    resolve_crate_path("rstest-bdd", "rstest_bdd")
}

/// Return a token stream pointing to the `rstest_bdd_harness` crate or its
/// renamed form.
pub(crate) fn rstest_bdd_harness_path() -> TokenStream2 {
    resolve_crate_path("rstest-bdd-harness", "rstest_bdd_harness")
}

fn resolve_crate_path(crate_name_str: &str, default_ident: &str) -> TokenStream2 {
    match crate_name(crate_name_str) {
        Ok(found) => {
            let ident = match found {
                FoundCrate::Itself => Ident::new(default_ident, Span::call_site()),
                FoundCrate::Name(name) => Ident::new(&name, Span::call_site()),
            };
            quote! { ::#ident }
        }
        Err(err) => handle_missing_crate(crate_name_str, &err),
    }
}

#[cfg(test)]
fn handle_missing_crate(crate_name_str: &str, _: &proc_macro_crate::Error) -> TokenStream2 {
    // Tests compile the macros crate in isolation without dependency crates, so
    // fall back to the default package name.
    let ident = Ident::new(&crate_name_str.replace('-', "_"), Span::call_site());
    quote! { ::#ident }
}

#[cfg(not(test))]
fn handle_missing_crate(crate_name_str: &str, err: &proc_macro_crate::Error) -> TokenStream2 {
    panic!("{crate_name_str} crate not found: {err}");
}

#[cfg(test)]
mod tests {
    use super::handle_missing_crate;
    use proc_macro_crate::Error;
    use std::path::PathBuf;

    #[test]
    fn returns_fallback_path_for_runtime_crate() {
        let error = Error::CrateNotFound {
            crate_name: "rstest-bdd".to_string(),
            path: PathBuf::new(),
        };
        let tokens = handle_missing_crate("rstest-bdd", &error);
        assert_eq!(tokens.to_string(), ":: rstest_bdd");
    }

    #[test]
    fn returns_fallback_path_for_harness_crate() {
        let error = Error::CrateNotFound {
            crate_name: "rstest-bdd-harness".to_string(),
            path: PathBuf::new(),
        };
        let tokens = handle_missing_crate("rstest-bdd-harness", &error);
        assert_eq!(tokens.to_string(), ":: rstest_bdd_harness");
    }
}
