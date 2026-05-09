//! Code generation utilities for the proc macros.
//!
//! This module emits fully-qualified paths (`::rstest_bdd::…`) so the macros crate
//! does not depend on the runtime crate at compile-time.

use proc_macro_crate::{FoundCrate, crate_name};
use proc_macro2::{Ident, Span, TokenStream as TokenStream2};
use quote::quote;

struct CrateSpec {
    package_name: &'static str,
    default_crate_name: &'static str,
    adapter_type_names: &'static [&'static str],
}

pub(crate) mod scenario;
pub(crate) mod wrapper;

const RSTEST_BDD: CrateSpec = CrateSpec {
    package_name: "rstest-bdd",
    default_crate_name: "rstest_bdd",
    adapter_type_names: &[],
};
const RSTEST_BDD_HARNESS: CrateSpec = CrateSpec {
    package_name: "rstest-bdd-harness",
    default_crate_name: "rstest_bdd_harness",
    adapter_type_names: &[],
};
const TOKIO_HARNESS: CrateSpec = CrateSpec {
    package_name: "rstest-bdd-harness-tokio",
    default_crate_name: "rstest_bdd_harness_tokio",
    adapter_type_names: &["TokioHarness", "TokioAttributePolicy"],
};
const GPUI_HARNESS: CrateSpec = CrateSpec {
    package_name: "rstest-bdd-harness-gpui",
    default_crate_name: "rstest_bdd_harness_gpui",
    adapter_type_names: &["GpuiHarness", "GpuiAttributePolicy"],
};

/// Return a token stream pointing to the `rstest_bdd` crate or its renamed form.
pub(crate) fn rstest_bdd_path() -> TokenStream2 {
    resolve_crate_path(&RSTEST_BDD)
}

/// Return a token stream pointing to the `rstest_bdd_harness` crate or its
/// renamed form.
pub(crate) fn rstest_bdd_harness_path() -> TokenStream2 {
    resolve_crate_path(&RSTEST_BDD_HARNESS)
}

/// Try to return a token stream pointing to the requested crate or renamed
/// dependency without panicking when the consumer does not depend on it.
fn try_resolve_crate_path(spec: &CrateSpec) -> Option<TokenStream2> {
    crate_name(spec.package_name)
        .ok()
        .map(|found| found_crate_path(found, spec))
}

/// Return a token stream pointing to the `rstest_bdd_harness_tokio` crate or
/// its renamed form.
///
/// Used by the `runtime = "tokio-current-thread"` compatibility alias to
/// resolve `TokioHarness` via proper crate lookup, supporting downstream
/// crates that rename the dependency in their `Cargo.toml`.
pub(crate) fn rstest_bdd_harness_tokio_path() -> TokenStream2 {
    resolve_crate_path(&TOKIO_HARNESS)
}

/// Return the crate root that provides base harness API for the given harness
/// or attribute-policy path.
pub(crate) fn rstest_bdd_harness_api_path_for(adapter_path: &syn::Path) -> TokenStream2 {
    first_party_adapter_api_path(adapter_path).unwrap_or_else(rstest_bdd_harness_path)
}

fn first_party_adapter_api_path(adapter_path: &syn::Path) -> Option<TokenStream2> {
    let root = adapter_path.segments.first()?;
    if first_party_adapter_path_matches(adapter_path, &TOKIO_HARNESS)
        || first_party_adapter_path_matches(adapter_path, &GPUI_HARNESS)
    {
        let root = &root.ident;
        Some(quote! { ::#root })
    } else {
        None
    }
}

fn first_party_adapter_path_matches(adapter_path: &syn::Path, spec: &CrateSpec) -> bool {
    path_last_ident_matches(adapter_path, spec.adapter_type_names)
        && path_root_matches_crate(adapter_path, spec)
}

fn path_last_ident_matches(path: &syn::Path, expected: &[&str]) -> bool {
    path.segments
        .last()
        .is_some_and(|segment| expected.iter().any(|name| segment.ident == name))
}

fn path_root_matches_crate(path: &syn::Path, spec: &CrateSpec) -> bool {
    let Some(root) = path.segments.first() else {
        return false;
    };
    if root.ident == spec.default_crate_name {
        return true;
    }
    let Some(crate_path) = try_resolve_crate_path(spec) else {
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

fn resolve_crate_path(spec: &CrateSpec) -> TokenStream2 {
    match crate_name(spec.package_name) {
        Ok(found) => found_crate_path(found, spec),
        Err(err) => handle_missing_crate(spec, &err),
    }
}

fn found_crate_path(found: FoundCrate, spec: &CrateSpec) -> TokenStream2 {
    let ident = match found {
        FoundCrate::Itself => Ident::new(spec.default_crate_name, Span::call_site()),
        FoundCrate::Name(name) => Ident::new(&name, Span::call_site()),
    };
    quote! { ::#ident }
}

#[cfg(test)]
fn handle_missing_crate(spec: &CrateSpec, _: &proc_macro_crate::Error) -> TokenStream2 {
    // Tests compile the macros crate in isolation without dependency crates, so
    // fall back to the default package name.
    let ident = Ident::new(spec.default_crate_name, Span::call_site());
    quote! { ::#ident }
}

#[cfg(not(test))]
fn handle_missing_crate(spec: &CrateSpec, err: &proc_macro_crate::Error) -> TokenStream2 {
    let crate_name = spec.package_name;
    panic!("{crate_name} crate not found: {err}");
}

#[cfg(test)]
mod tests {
    use super::{
        GPUI_HARNESS, RSTEST_BDD, RSTEST_BDD_HARNESS, TOKIO_HARNESS, handle_missing_crate,
    };
    use proc_macro_crate::Error;
    use std::path::PathBuf;

    fn not_found_error(crate_name: &str) -> Error {
        Error::CrateNotFound {
            crate_name: crate_name.to_string(),
            path: PathBuf::new(),
        }
    }

    #[test]
    fn returns_fallback_path_for_runtime_crate() {
        let tokens = handle_missing_crate(&RSTEST_BDD, &not_found_error("rstest-bdd"));
        assert_eq!(tokens.to_string(), ":: rstest_bdd");
    }

    #[test]
    fn returns_fallback_path_for_harness_crate() {
        let tokens =
            handle_missing_crate(&RSTEST_BDD_HARNESS, &not_found_error("rstest-bdd-harness"));
        assert_eq!(tokens.to_string(), ":: rstest_bdd_harness");
    }

    #[test]
    fn returns_fallback_path_for_harness_tokio_crate() {
        let tokens =
            handle_missing_crate(&TOKIO_HARNESS, &not_found_error("rstest-bdd-harness-tokio"));
        assert_eq!(tokens.to_string(), ":: rstest_bdd_harness_tokio");
    }

    #[test]
    fn returns_fallback_path_for_harness_gpui_crate() {
        let tokens =
            handle_missing_crate(&GPUI_HARNESS, &not_found_error("rstest-bdd-harness-gpui"));
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
