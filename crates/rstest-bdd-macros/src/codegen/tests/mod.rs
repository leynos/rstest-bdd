//! Unit tests for shared code generation utilities.

use std::path::PathBuf;

use proc_macro_crate::Error;
use proptest::prelude::*;
use rstest::rstest;

use super::{
    GPUI_HARNESS,
    HarnessApiResolution,
    RSTEST_BDD,
    RSTEST_BDD_HARNESS,
    TOKIO_HARNESS,
    default_resolver,
    handle_missing_crate,
    resolve_harness_api_with,
};

fn not_found_error(crate_name: &str) -> Error {
    Error::CrateNotFound {
        crate_name: crate_name.to_owned(),
        path: PathBuf::new(),
    }
}

/// Resolve one adapter path with the unit-test resolver.
fn resolve_harness_api(adapter_path: &syn::Path) -> HarnessApiResolution {
    resolve_harness_api_with(adapter_path, default_resolver())
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
    let resolution = resolve_harness_api(&adapter_path);
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
fn pure_resolution_selects_path_and_diagnostic_state(
    #[case] adapter_path: &str,
    #[case] expected_path: &str,
    #[case] has_finding: bool,
) {
    let adapter_path = parse_path(adapter_path);
    let resolution = resolve_harness_api(&adapter_path);
    let findings =
        super::SharedPolicyDiagnostics::for_scenario(Some(&adapter_path), None).findings();

    assert_eq!(resolution.api_path.to_string(), expected_path);
    assert_eq!(
        findings.harness_unrecognized_path, has_finding,
        "the harness slot finding must match the expectation"
    );
}

#[test]
fn matching_type_name_under_unknown_root_uses_base_harness_crate() {
    let harness_path = syn::parse_quote!(my_harness::TokioHarness);
    let resolution = resolve_harness_api(&harness_path);
    assert_eq!(resolution.api_path.to_string(), ":: rstest_bdd_harness");
}

#[test]
fn aliased_import_falls_back_to_base_harness() {
    // Simulates: use rstest_bdd_harness_tokio::TokioHarness as TH;
    // #[scenario(harness = my_mod::TH)] - type alias not in known names.
    let harness_path = syn::parse_quote!(rstest_bdd_harness_tokio::SomeAlias);
    let tokens = resolve_harness_api(&harness_path).api_path;
    // The type name is not in TOKIO_HARNESS.adapter_type_names, so fall back.
    assert_eq!(tokens.to_string(), ":: rstest_bdd_harness");
}

#[test]
fn renamed_root_with_known_type_uses_test_only_base_harness_fallback() {
    // Simulates: tok = { package = "rstest-bdd-harness-tokio" }
    // #[scenario(harness = tok::TokioHarness)]
    // In a test build the resolver reports no renamed root, so no root match.
    let harness_path = syn::parse_quote!(tok::TokioHarness);
    let tokens = resolve_harness_api(&harness_path).api_path;
    assert_eq!(tokens.to_string(), ":: rstest_bdd_harness");
}

#[test]
fn custom_harness_api_path_uses_base_harness_crate() {
    let harness_path = syn::parse_quote!(my_harness::Harness);
    let tokens = resolve_harness_api(&harness_path).api_path;
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

        let resolver = super::default_resolver();
        prop_assert!(super::path_root_matches_crate(&matching_path, spec, resolver));
        prop_assert!(!super::path_root_matches_crate(&renamed_path, spec, resolver));
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

        let resolver = super::default_resolver();
        prop_assert!(!super::first_party_adapter_path_matches(&imported_path, spec, resolver));
        prop_assert!(super::first_party_adapter_path_matches(&canonical_path, spec, resolver));
        prop_assert!(!super::first_party_adapter_path_matches(&renamed_path, spec, resolver));
        prop_assert!(!super::first_party_adapter_path_matches(&aliased_path, spec, resolver));
    }
}
