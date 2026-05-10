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
    first_party_adapter_spec(adapter_path)
        .map(|spec| first_party_adapter_api_root(adapter_path, spec))
}

fn first_party_adapter_spec(adapter_path: &syn::Path) -> Option<&'static CrateSpec> {
    [&TOKIO_HARNESS, &GPUI_HARNESS]
        .into_iter()
        .find(|spec| first_party_adapter_path_matches(adapter_path, spec))
}

fn first_party_adapter_api_root(adapter_path: &syn::Path, spec: &CrateSpec) -> TokenStream2 {
    if path_root_matches_crate(adapter_path, spec) {
        let Some(root) = adapter_path.segments.first().map(|segment| &segment.ident) else {
            return resolve_crate_path(spec);
        };
        quote! { ::#root }
    } else {
        resolve_crate_path(spec)
    }
}

fn first_party_adapter_path_matches(adapter_path: &syn::Path, spec: &CrateSpec) -> bool {
    path_last_ident_matches(adapter_path, spec.adapter_type_names)
        && (path_root_matches_crate(adapter_path, spec)
            || is_imported_adapter_type_path(adapter_path))
}

fn is_imported_adapter_type_path(path: &syn::Path) -> bool {
    path.segments.len() == 1
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
    use rstest::rstest;
    use std::path::PathBuf;

    fn not_found_error(crate_name: &str) -> Error {
        Error::CrateNotFound {
            crate_name: crate_name.to_string(),
            path: PathBuf::new(),
        }
    }

    #[expect(clippy::expect_used, reason = "test path literals should parse")]
    fn parse_path(path: &str) -> syn::Path {
        syn::parse_str(path).expect("parse path")
    }

    #[rstest]
    #[case::runtime(&RSTEST_BDD, "rstest-bdd", ":: rstest_bdd")]
    #[case::harness(
        &RSTEST_BDD_HARNESS,
        "rstest-bdd-harness",
        ":: rstest_bdd_harness"
    )]
    #[case::tokio(
        &TOKIO_HARNESS,
        "rstest-bdd-harness-tokio",
        ":: rstest_bdd_harness_tokio"
    )]
    #[case::gpui(
        &GPUI_HARNESS,
        "rstest-bdd-harness-gpui",
        ":: rstest_bdd_harness_gpui"
    )]
    fn returns_fallback_path_for_known_crates(
        #[case] crate_spec: &super::CrateSpec,
        #[case] not_found_name: &str,
        #[case] expected_string: &str,
    ) {
        let tokens = handle_missing_crate(crate_spec, &not_found_error(not_found_name));
        assert_eq!(tokens.to_string(), expected_string);
    }

    #[rstest]
    #[case::tokio_harness_canonical(
        "rstest_bdd_harness_tokio::TokioHarness",
        ":: rstest_bdd_harness_tokio"
    )]
    #[case::tokio_harness_imported("TokioHarness", ":: rstest_bdd_harness_tokio")]
    #[case::tokio_policy_imported("TokioAttributePolicy", ":: rstest_bdd_harness_tokio")]
    #[case::gpui_harness_imported("GpuiHarness", ":: rstest_bdd_harness_gpui")]
    #[case::gpui_policy_canonical(
        "rstest_bdd_harness_gpui::GpuiAttributePolicy",
        ":: rstest_bdd_harness_gpui"
    )]
    #[case::gpui_policy_imported("GpuiAttributePolicy", ":: rstest_bdd_harness_gpui")]
    fn first_party_adapter_api_path_uses_adapter_crate(
        #[case] adapter_path: &str,
        #[case] expected: &str,
    ) {
        let adapter_path = parse_path(adapter_path);
        let tokens = super::rstest_bdd_harness_api_path_for(&adapter_path);
        assert_eq!(tokens.to_string(), expected);
    }

    #[test]
    fn matching_type_name_under_unknown_root_uses_base_harness_crate() {
        let harness_path = syn::parse_quote!(my_harness::TokioHarness);
        let tokens = super::rstest_bdd_harness_api_path_for(&harness_path);
        assert_eq!(tokens.to_string(), ":: rstest_bdd_harness");
    }

    #[test]
    fn custom_harness_api_path_uses_base_harness_crate() {
        let harness_path = syn::parse_quote!(my_harness::Harness);
        let tokens = super::rstest_bdd_harness_api_path_for(&harness_path);
        assert_eq!(tokens.to_string(), ":: rstest_bdd_harness");
    }
}
