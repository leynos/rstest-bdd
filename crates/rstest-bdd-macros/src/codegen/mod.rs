//! Code generation utilities for the proc macros.
//!
//! This module emits fully-qualified paths (`::rstest_bdd::…`) so the macros crate
//! does not depend on the runtime crate at compile-time.

use proc_macro2::{Ident, Span, TokenStream as TokenStream2};
use proc_macro_crate::{crate_name, FoundCrate};
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
        Err(err) => handle_missing_runtime(&err),
    }
}

#[cfg(test)]
fn handle_missing_runtime(_: &proc_macro_crate::Error) -> TokenStream2 {
    // Tests compile the macros crate in isolation without the runtime crate, so
    // fall back to the default package name.
    quote! { ::rstest_bdd }
}

#[cfg(not(test))]
fn handle_missing_runtime(err: &proc_macro_crate::Error) -> TokenStream2 {
    panic!("rstest-bdd crate not found: {err}");
}

#[cfg(test)]
mod tests {
    use super::handle_missing_runtime;
    use proc_macro_crate::Error;
    use std::path::PathBuf;

    #[test]
    fn returns_fallback_path_in_tests() {
        let error = Error::CrateNotFound {
            crate_name: "rstest-bdd".to_string(),
            path: PathBuf::new(),
        };
        let tokens = handle_missing_runtime(&error);
        assert_eq!(tokens.to_string(), ":: rstest_bdd");
    }
}
