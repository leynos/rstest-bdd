//! Code generation utilities for the proc macros.
//!
//! This module emits fully-qualified paths (`::rstest_bdd::…`) so the macros crate
//! does not depend on the runtime crate at compile-time.

use proc_macro_crate::{FoundCrate, crate_name};
use proc_macro2::{Ident, Span, TokenStream as TokenStream2};
use quote::quote;
use rstest_bdd_policy::TestAttributeHint;
/// Package and crate-name metadata used while resolving generated paths.
struct CrateSpec {
    /// Cargo package name supplied to `proc_macro_crate`.
    package_name: &'static str,
    /// Default Rust crate identifier when the package is not renamed.
    default_crate_name: &'static str,
    /// Adapter type names that establish first-party adapter evidence.
    adapter_type_names: &'static [&'static str],
}
mod adapter_fallback;
pub(crate) mod scenario;
pub(crate) mod tracking;
pub(crate) mod wrapper;
pub(crate) use adapter_fallback::SharedAdapterResolutions;
use adapter_fallback::{AdapterFallback, fallback_candidate};
/// Specification for the core `rstest-bdd` runtime crate.
const RSTEST_BDD: CrateSpec = CrateSpec {
    package_name: "rstest-bdd",
    default_crate_name: "rstest_bdd",
    adapter_type_names: &[],
};
/// Specification for the base `rstest-bdd-harness` crate.
const RSTEST_BDD_HARNESS: CrateSpec = CrateSpec {
    package_name: "rstest-bdd-harness",
    default_crate_name: "rstest_bdd_harness",
    adapter_type_names: &[],
};
/// Specification for the first-party Tokio harness adapter.
const TOKIO_HARNESS: CrateSpec = CrateSpec {
    package_name: "rstest-bdd-harness-tokio",
    default_crate_name: "rstest_bdd_harness_tokio",
    adapter_type_names: &["TokioHarness", "TokioAttributePolicy"],
};
/// Specification for the first-party GPUI harness adapter.
const GPUI_HARNESS: CrateSpec = CrateSpec {
    package_name: "rstest-bdd-harness-gpui",
    default_crate_name: "rstest_bdd_harness_gpui",
    adapter_type_names: &["GpuiHarness", "GpuiAttributePolicy"],
};
/// Return a token stream pointing to the `rstest_bdd` crate or its renamed form.
pub(crate) fn rstest_bdd_path() -> TokenStream2 { resolve_crate_path(&RSTEST_BDD) }
/// Return a token stream pointing to the `rstest_bdd_harness` crate or its
/// renamed form.
pub(crate) fn rstest_bdd_harness_path() -> TokenStream2 { resolve_crate_path(&RSTEST_BDD_HARNESS) }
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
pub(crate) fn rstest_bdd_harness_tokio_path() -> TokenStream2 { resolve_crate_path(&TOKIO_HARNESS) }
/// Return the crate root that provides base harness API for the given harness
/// or attribute-policy path.
///
/// Adapter detection relies on the crate-root identifier in `adapter_path`
/// matching either the default snake-case crate name or a resolved renamed
/// dependency. Renamed dependencies that cannot be resolved at
/// macro-expansion time (including test builds) fall back to the base
/// `rstest-bdd-harness` path.
#[derive(Clone)]
pub(crate) struct HarnessApiResolution {
    /// Base harness API crate path selected for generated code.
    pub(crate) api_path: TokenStream2,
    /// Optional metadata used to emit the fallback diagnostic at the boundary.
    fallback: Option<AdapterFallback>,
}

/// Resolve the base harness API path and any qualifying fallback metadata.
///
/// This operation is pure: callers decide where to emit the optional
/// diagnostic so one resolution can be reused throughout code generation.
pub(crate) fn resolve_harness_api(adapter_path: &syn::Path) -> HarnessApiResolution {
    if let Some(api_path) = first_party_adapter_api_path(adapter_path) {
        return HarnessApiResolution {
            api_path,
            fallback: None,
        };
    }
    HarnessApiResolution {
        api_path: rstest_bdd_harness_path(),
        fallback: fallback_candidate(adapter_path),
    }
}

/// Emit or generate the diagnostic selected for one fallback resolution.
pub(crate) fn first_party_adapter_fallback_diagnostic(
    resolution: &HarnessApiResolution,
) -> TokenStream2 {
    adapter_fallback::emit_first_party_adapter_fallback_warning(resolution.fallback.as_ref());
    adapter_fallback::first_party_adapter_fallback_warning_tokens(resolution.fallback.as_ref())
}

/// Return the generated-test attribute hint for a recognized first-party adapter.
pub(crate) fn first_party_adapter_attribute_hint(
    adapter_path: &syn::Path,
) -> Option<TestAttributeHint> {
    if first_party_adapter_path_matches(adapter_path, &TOKIO_HARNESS) {
        Some(TestAttributeHint::RstestWithTokioCurrentThread)
    } else if first_party_adapter_path_matches(adapter_path, &GPUI_HARNESS) {
        Some(TestAttributeHint::RstestWithGpuiTest)
    } else {
        None
    }
}

/// Resolve a recognized first-party adapter path to its API crate root.
fn first_party_adapter_api_path(adapter_path: &syn::Path) -> Option<TokenStream2> {
    first_party_adapter_spec(adapter_path)
        .map(|spec| first_party_adapter_api_root(adapter_path, spec))
}

/// Return the first-party adapter specification matching a supplied path.
fn first_party_adapter_spec(adapter_path: &syn::Path) -> Option<&'static CrateSpec> {
    [&TOKIO_HARNESS, &GPUI_HARNESS]
        .into_iter()
        .find(|spec| first_party_adapter_path_matches(adapter_path, spec))
}
/// Select the API root for a recognized adapter path.
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

/// Determine whether a path has sufficient evidence for a first-party adapter.
fn first_party_adapter_path_matches(adapter_path: &syn::Path, spec: &CrateSpec) -> bool {
    path_last_ident_matches(adapter_path, spec.adapter_type_names)
        && (path_root_matches_crate(adapter_path, spec)
            || is_imported_adapter_type_path(adapter_path, spec))
}

/// Determine whether an unqualified imported adapter type can be resolved.
fn is_imported_adapter_type_path(path: &syn::Path, spec: &CrateSpec) -> bool {
    path.segments.len() == 1 && try_resolve_crate_path(spec).is_some()
}

/// Determine whether the final path segment is one of the expected identifiers.
fn path_last_ident_matches(path: &syn::Path, expected: &[&str]) -> bool {
    path.segments
        .last()
        .is_some_and(|segment| expected.iter().any(|name| segment.ident == name))
}

/// Determine whether the root segment names a first-party adapter crate.
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

/// Resolve a dependency specification to a generated crate path.
fn resolve_crate_path(spec: &CrateSpec) -> TokenStream2 {
    match crate_name(spec.package_name) {
        Ok(found) => found_crate_path(found, spec),
        Err(err) => handle_missing_crate(spec, &err),
    }
}

/// Convert a resolved dependency into a fully-qualified token path.
fn found_crate_path(found: FoundCrate, spec: &CrateSpec) -> TokenStream2 {
    let ident = match found {
        FoundCrate::Itself => Ident::new(spec.default_crate_name, Span::call_site()),
        FoundCrate::Name(name) => Ident::new(&name, Span::call_site()),
    };
    quote! { ::#ident }
}

#[cfg(test)]
/// Provide a deterministic default crate path for isolated macro tests.
fn handle_missing_crate(spec: &CrateSpec, _: &proc_macro_crate::Error) -> TokenStream2 {
    // Tests compile the macros crate in isolation without dependency crates, so
    // fall back to the default package name.
    let ident = Ident::new(spec.default_crate_name, Span::call_site());
    quote! { ::#ident }
}

#[cfg(not(test))]
/// Abort expansion when a required generated-code dependency is unavailable.
fn handle_missing_crate(spec: &CrateSpec, err: &proc_macro_crate::Error) -> TokenStream2 {
    let crate_name = spec.package_name;
    panic!("{crate_name} crate not found: {err}");
}

#[cfg(test)]
mod tests {
    //! Unit tests for shared code generation utilities.

    use std::path::PathBuf;

    use proc_macro_crate::Error;
    use proptest::prelude::*;
    use rstest::rstest;

    use super::{
        GPUI_HARNESS,
        RSTEST_BDD,
        RSTEST_BDD_HARNESS,
        TOKIO_HARNESS,
        handle_missing_crate,
    };

    fn not_found_error(crate_name: &str) -> Error {
        Error::CrateNotFound {
            crate_name: crate_name.to_owned(),
            path: PathBuf::new(),
        }
    }

    fn parse_path(path: &str) -> syn::Path {
        match syn::parse_str(path) {
            Ok(parsed) => parsed,
            Err(err) => panic!("parse path {path}: {err}"),
        }
    }

    fn adapter_spec(is_tokio: bool) -> &'static super::CrateSpec {
        if is_tokio {
            &TOKIO_HARNESS
        } else {
            &GPUI_HARNESS
        }
    }

    fn known_adapter_type(spec: &super::CrateSpec, use_policy_type: bool) -> &'static str {
        let [harness_type, policy_type] = spec.adapter_type_names else {
            panic!("first-party adapter specs have harness and policy type names");
        };
        if use_policy_type {
            policy_type
        } else {
            harness_type
        }
    }

    #[rstest]
    #[case(&RSTEST_BDD, "rstest-bdd", ":: rstest_bdd")]
    #[case(&RSTEST_BDD_HARNESS, "rstest-bdd-harness", ":: rstest_bdd_harness")]
    #[case(&TOKIO_HARNESS, "rstest-bdd-harness-tokio", ":: rstest_bdd_harness_tokio")]
    #[case(&GPUI_HARNESS, "rstest-bdd-harness-gpui", ":: rstest_bdd_harness_gpui")]
    fn returns_fallback_path(
        #[case] spec: &super::CrateSpec,
        #[case] pkg: &str,
        #[case] expected: &str,
    ) {
        let tokens = handle_missing_crate(spec, &not_found_error(pkg));
        assert_eq!(tokens.to_string(), expected);
    }

    #[rstest]
    #[case::tokio_harness_canonical(
        "rstest_bdd_harness_tokio::TokioHarness",
        ":: rstest_bdd_harness_tokio"
    )]
    #[case::tokio_harness_imported("TokioHarness", ":: rstest_bdd_harness")]
    #[case::tokio_policy_imported("TokioAttributePolicy", ":: rstest_bdd_harness")]
    #[case::gpui_harness_imported("GpuiHarness", ":: rstest_bdd_harness")]
    #[case::gpui_policy_canonical(
        "rstest_bdd_harness_gpui::GpuiAttributePolicy",
        ":: rstest_bdd_harness_gpui"
    )]
    #[case::gpui_policy_imported("GpuiAttributePolicy", ":: rstest_bdd_harness")]
    fn adapter_api_path_uses_expected_crate(#[case] adapter_path: &str, #[case] expected: &str) {
        let adapter_path = parse_path(adapter_path);
        let resolution = super::resolve_harness_api(&adapter_path);
        assert_eq!(resolution.api_path.to_string(), expected);
    }

    #[rstest]
    #[case::aliased_tokio(
        "alias::rstest_bdd_harness_tokio::TokioHarness",
        ":: rstest_bdd_harness",
        true
    )]
    #[case::canonical_tokio(
        "rstest_bdd_harness_tokio::TokioHarness",
        ":: rstest_bdd_harness_tokio",
        false
    )]
    #[case::canonical_gpui(
        "rstest_bdd_harness_gpui::GpuiAttributePolicy",
        ":: rstest_bdd_harness_gpui",
        false
    )]
    #[case::custom_tokio_name("custom::TokioHarness", ":: rstest_bdd_harness", false)]
    fn pure_resolution_selects_path_and_fallback_metadata(
        #[case] adapter_path: &str,
        #[case] expected_path: &str,
        #[case] has_fallback: bool,
    ) {
        let resolution = super::resolve_harness_api(&parse_path(adapter_path));

        assert_eq!(resolution.api_path.to_string(), expected_path);
        assert_eq!(resolution.fallback.is_some(), has_fallback);
    }

    #[test]
    fn matching_type_name_under_unknown_root_uses_base_harness_crate() {
        let harness_path = syn::parse_quote!(my_harness::TokioHarness);
        let resolution = super::resolve_harness_api(&harness_path);
        assert_eq!(resolution.api_path.to_string(), ":: rstest_bdd_harness");
    }

    #[test]
    fn aliased_import_falls_back_to_base_harness() {
        // Simulates: use rstest_bdd_harness_tokio::TokioHarness as TH;
        // #[scenario(harness = my_mod::TH)] - type alias not in known names.
        let harness_path = syn::parse_quote!(rstest_bdd_harness_tokio::SomeAlias);
        let tokens = super::resolve_harness_api(&harness_path).api_path;
        // The type name is not in TOKIO_HARNESS.adapter_type_names, so fall back.
        assert_eq!(tokens.to_string(), ":: rstest_bdd_harness");
    }

    #[test]
    fn renamed_root_with_known_type_uses_test_only_base_harness_fallback() {
        // Simulates: tok = { package = "rstest-bdd-harness-tokio" }
        // #[scenario(harness = tok::TokioHarness)]
        // In a test build try_resolve_crate_path returns None, so no root match.
        let harness_path = syn::parse_quote!(tok::TokioHarness);
        let tokens = super::resolve_harness_api(&harness_path).api_path;
        assert_eq!(tokens.to_string(), ":: rstest_bdd_harness");
    }

    #[test]
    fn custom_harness_api_path_uses_base_harness_crate() {
        let harness_path = syn::parse_quote!(my_harness::Harness);
        let tokens = super::resolve_harness_api(&harness_path).api_path;
        assert_eq!(tokens.to_string(), ":: rstest_bdd_harness");
    }

    proptest! {
        #[test]
        fn path_root_matches_crate_depends_on_resolved_root(
            is_tokio in any::<bool>(),
            suffix in any::<u16>(),
            use_policy_type in any::<bool>(),
        ) {
            let spec = adapter_spec(is_tokio);
            let known_type = known_adapter_type(spec, use_policy_type);
            let matching_path = parse_path(&format!("{}::{known_type}", spec.default_crate_name));
            let renamed_path = parse_path(&format!("renamed_{suffix}::{known_type}"));

            prop_assert!(super::path_root_matches_crate(&matching_path, spec));
            prop_assert!(!super::path_root_matches_crate(&renamed_path, spec));
        }

        #[test]
        fn first_party_adapter_path_matches_requires_known_type_and_valid_root(
            is_tokio in any::<bool>(),
            suffix in any::<u16>(),
            use_policy_type in any::<bool>(),
        ) {
            let spec = adapter_spec(is_tokio);
            let known_type = known_adapter_type(spec, use_policy_type);
            let unknown_type = format!("Alias{suffix}");
            let imported_path = parse_path(known_type);
            let canonical_path = parse_path(&format!("{}::{known_type}", spec.default_crate_name));
            let renamed_path = parse_path(&format!("renamed_{suffix}::{known_type}"));
            let aliased_path = parse_path(&format!("{}::{unknown_type}", spec.default_crate_name));

            prop_assert!(!super::first_party_adapter_path_matches(&imported_path, spec));
            prop_assert!(super::first_party_adapter_path_matches(&canonical_path, spec));
            prop_assert!(!super::first_party_adapter_path_matches(&renamed_path, spec));
            prop_assert!(!super::first_party_adapter_path_matches(&aliased_path, spec));
        }
    }
}
