//! Code generation utilities for the proc macros.
//!
//! This module emits fully-qualified paths (`::rstest_bdd::…`) so the macros crate
//! does not depend on the runtime crate at compile-time.

use proc_macro_crate::{FoundCrate, crate_name};
use proc_macro2::{Ident, Span, TokenStream as TokenStream2};
use quote::quote;

pub(crate) mod scenario;
pub(crate) mod wrapper;

const TOKIO_HARNESS_PACKAGE: &str = "rstest-bdd-harness-tokio";
const TOKIO_HARNESS_CRATE: &str = "rstest_bdd_harness_tokio";
const TOKIO_HARNESS_TYPES: &[&str] = &["TokioHarness", "TokioAttributePolicy"];
const GPUI_HARNESS_PACKAGE: &str = "rstest-bdd-harness-gpui";
const GPUI_HARNESS_CRATE: &str = "rstest_bdd_harness_gpui";
const GPUI_HARNESS_TYPES: &[&str] = &["GpuiHarness", "GpuiAttributePolicy"];

/// Return a token stream pointing to the `rstest_bdd` crate or its renamed form.
pub(crate) fn rstest_bdd_path() -> TokenStream2 {
    resolve_crate_path("rstest-bdd", "rstest_bdd")
}

/// Return a token stream pointing to the `rstest_bdd_harness` crate or its
/// renamed form.
pub(crate) fn rstest_bdd_harness_path() -> TokenStream2 {
    resolve_crate_path("rstest-bdd-harness", "rstest_bdd_harness")
}

/// Try to return a token stream pointing to the requested crate or renamed
/// dependency without panicking when the consumer does not depend on it.
pub(crate) fn try_resolve_crate_path(
    crate_name_str: &str,
    default_ident: &str,
) -> Option<TokenStream2> {
    crate_name(crate_name_str)
        .ok()
        .map(|found| found_crate_path(found, default_ident))
}

/// Return a token stream pointing to the `rstest_bdd_harness_tokio` crate or
/// its renamed form.
///
/// Used by the `runtime = "tokio-current-thread"` compatibility alias to
/// resolve `TokioHarness` via proper crate lookup, supporting downstream
/// crates that rename the dependency in their `Cargo.toml`.
pub(crate) fn rstest_bdd_harness_tokio_path() -> TokenStream2 {
    resolve_crate_path("rstest-bdd-harness-tokio", "rstest_bdd_harness_tokio")
}

/// Return the crate root that provides base harness API for the given harness
/// or attribute-policy path.
pub(crate) fn rstest_bdd_harness_api_path_for(adapter_path: &syn::Path) -> TokenStream2 {
    first_party_adapter_api_path(adapter_path).unwrap_or_else(rstest_bdd_harness_path)
}

fn first_party_adapter_api_path(adapter_path: &syn::Path) -> Option<TokenStream2> {
    let root = adapter_path.segments.first()?;
    if first_party_adapter_path_matches(
        adapter_path,
        TOKIO_HARNESS_PACKAGE,
        TOKIO_HARNESS_CRATE,
        TOKIO_HARNESS_TYPES,
    ) || first_party_adapter_path_matches(
        adapter_path,
        GPUI_HARNESS_PACKAGE,
        GPUI_HARNESS_CRATE,
        GPUI_HARNESS_TYPES,
    ) {
        let root = &root.ident;
        Some(quote! { ::#root })
    } else {
        None
    }
}

fn first_party_adapter_path_matches(
    adapter_path: &syn::Path,
    package_name: &str,
    default_crate_name: &str,
    adapter_type_names: &[&str],
) -> bool {
    path_last_ident_matches(adapter_path, adapter_type_names)
        && path_root_matches_crate(adapter_path, package_name, default_crate_name)
}

fn path_last_ident_matches(path: &syn::Path, expected: &[&str]) -> bool {
    path.segments
        .last()
        .is_some_and(|segment| expected.iter().any(|name| segment.ident == name))
}

fn path_root_matches_crate(path: &syn::Path, package_name: &str, default_crate_name: &str) -> bool {
    let Some(root) = path.segments.first() else {
        return false;
    };
    if root.ident == default_crate_name {
        return true;
    }
    let Some(crate_path) = try_resolve_crate_path(package_name, default_crate_name) else {
        return false;
    };
    let Ok(crate_path) = syn::parse2::<syn::Path>(crate_path) else {
        return false;
    };
    crate_path
        .segments
        .first()
        .is_some_and(|crate_root| crate_root.ident == root.ident)
}

fn resolve_crate_path(crate_name_str: &str, default_ident: &str) -> TokenStream2 {
    match crate_name(crate_name_str) {
        Ok(found) => found_crate_path(found, default_ident),
        Err(err) => handle_missing_crate(crate_name_str, &err),
    }
}

fn found_crate_path(found: FoundCrate, default_ident: &str) -> TokenStream2 {
    let ident = match found {
        FoundCrate::Itself => Ident::new(default_ident, Span::call_site()),
        FoundCrate::Name(name) => Ident::new(&name, Span::call_site()),
    };
    quote! { ::#ident }
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

    #[test]
    fn returns_fallback_path_for_harness_tokio_crate() {
        let error = Error::CrateNotFound {
            crate_name: "rstest-bdd-harness-tokio".to_string(),
            path: PathBuf::new(),
        };
        let tokens = handle_missing_crate("rstest-bdd-harness-tokio", &error);
        assert_eq!(tokens.to_string(), ":: rstest_bdd_harness_tokio");
    }

    #[test]
    fn returns_fallback_path_for_harness_gpui_crate() {
        let error = Error::CrateNotFound {
            crate_name: "rstest-bdd-harness-gpui".to_string(),
            path: PathBuf::new(),
        };
        let tokens = handle_missing_crate("rstest-bdd-harness-gpui", &error);
        assert_eq!(tokens.to_string(), ":: rstest_bdd_harness_gpui");
    }

    #[test]
    fn first_party_harness_api_path_uses_adapter_crate() {
        let harness_path = syn::parse_quote!(rstest_bdd_harness_tokio::TokioHarness);
        let tokens = super::rstest_bdd_harness_api_path_for(&harness_path);
        assert_eq!(tokens.to_string(), ":: rstest_bdd_harness_tokio");
    }

    #[test]
    fn first_party_attribute_api_path_uses_adapter_crate() {
        let policy_path = syn::parse_quote!(rstest_bdd_harness_gpui::GpuiAttributePolicy);
        let tokens = super::rstest_bdd_harness_api_path_for(&policy_path);
        assert_eq!(tokens.to_string(), ":: rstest_bdd_harness_gpui");
    }

    #[test]
    fn custom_harness_api_path_uses_base_harness_crate() {
        let harness_path = syn::parse_quote!(my_harness::Harness);
        let tokens = super::rstest_bdd_harness_api_path_for(&harness_path);
        assert_eq!(tokens.to_string(), ":: rstest_bdd_harness");
    }
}
