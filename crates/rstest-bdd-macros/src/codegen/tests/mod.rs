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

/// The error Cargo raises for a crate the caller does not depend on.
///
/// Derived from the spec so the helper takes a crate description rather than
/// a bare string.
fn not_found_error(spec: &super::CrateSpec) -> Error {
    Error::CrateNotFound {
        crate_name: spec.package_name.to_owned(),
        path: PathBuf::new(),
    }
}

/// Resolve one adapter path with the unit-test resolver.
fn resolve_harness_api(adapter_path: &syn::Path) -> HarnessApiResolution {
    resolve_harness_api_with(adapter_path, default_resolver())
}

/// A test path, parsed once so helpers take a path rather than a string.
struct TestPath(syn::Path);

impl TestPath {
    /// Parse `source` into a path, naming it if it does not parse.
    fn new(source: &str) -> Self {
        match syn::parse_str(source) {
            Ok(path) => Self(path),
            Err(err) => panic!("parse path {source}: {err}"),
        }
    }

    /// The parsed path.
    fn parsed(&self) -> &syn::Path { &self.0 }
}

/// Every first-party adapter spec, with the two type names each exports.
const SPECS: [(&super::CrateSpec, &str, &str); 2] = [
    (&TOKIO_HARNESS, "TokioHarness", "TokioAttributePolicy"),
    (&GPUI_HARNESS, "GpuiHarness", "GpuiAttributePolicy"),
];

#[test]
fn returns_fallback_path() {
    for (spec, expected) in [
        (&RSTEST_BDD, ":: rstest_bdd"),
        (&RSTEST_BDD_HARNESS, ":: rstest_bdd_harness"),
        (&TOKIO_HARNESS, ":: rstest_bdd_harness_tokio"),
        (&GPUI_HARNESS, ":: rstest_bdd_harness_gpui"),
    ] {
        let tokens = handle_missing_crate(spec, &not_found_error(spec));
        assert_eq!(
            tokens.to_string(),
            expected,
            "{} must fall back to its own crate root",
            spec.package_name
        );
    }
}

#[test]
fn a_missing_crate_names_itself_in_the_error() {
    let error = not_found_error(&RSTEST_BDD_HARNESS);
    let Error::CrateNotFound { crate_name, .. } = error else {
        panic!("the helper must build the not-found variant");
    };

    assert_eq!(
        crate_name, "rstest-bdd-harness",
        "the error must carry the crate it looked for"
    );
}

/// A table row for the API-path cases.
struct ApiPathCase {
    /// The supplied adapter path.
    adapter: TestPath,
    /// The crate path generated code must use.
    expected: &'static str,
}

impl ApiPathCase {
    /// Build one row from a source path and its expected crate path.
    fn new(source: &str, expected: &'static str) -> Self {
        Self {
            adapter: TestPath::new(source),
            expected,
        }
    }
}

#[test]
fn adapter_api_path_uses_expected_crate() {
    for case in [
        ApiPathCase::new(
            "rstest_bdd_harness_tokio::TokioHarness",
            ":: rstest_bdd_harness_tokio",
        ),
        ApiPathCase::new("TokioHarness", ":: rstest_bdd_harness"),
        ApiPathCase::new("TokioAttributePolicy", ":: rstest_bdd_harness"),
        ApiPathCase::new("GpuiHarness", ":: rstest_bdd_harness"),
        ApiPathCase::new(
            "rstest_bdd_harness_gpui::GpuiAttributePolicy",
            ":: rstest_bdd_harness_gpui",
        ),
        ApiPathCase::new("GpuiAttributePolicy", ":: rstest_bdd_harness"),
    ] {
        let resolution = resolve_harness_api(case.adapter.parsed());
        assert_eq!(
            resolution.api_path.to_string(),
            case.expected,
            "the generated crate path must match the case"
        );
    }
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
    let adapter_path = TestPath::new(adapter_path);
    let resolution = resolve_harness_api(adapter_path.parsed());
    let findings = super::SharedPolicyDiagnostics::for_scenario(
        Some(adapter_path.parsed()),
        None,
        false,
        false,
        false,
    )
    .findings();

    assert_eq!(resolution.api_path.to_string(), expected_path);
    assert_eq!(
        findings.slots.harness, has_finding,
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
    fn path_root_matches_crate_depends_on_resolved_root(suffix in any::<u16>()) {
        let resolver = super::default_resolver();
        for (spec, harness_type, policy_type) in SPECS {
            for known_type in [harness_type, policy_type] {
                let matching =
                    TestPath::new(&format!("{}::{known_type}", spec.default_crate_name));
                let renamed = TestPath::new(&format!("renamed_{suffix}::{known_type}"));

                prop_assert!(super::path_root_matches_crate(matching.parsed(), spec, resolver));
                prop_assert!(!super::path_root_matches_crate(renamed.parsed(), spec, resolver));
            }
        }
    }

    #[test]
    fn first_party_adapter_path_matches_requires_known_type_and_valid_root(
        suffix in any::<u16>(),
    ) {
        let resolver = super::default_resolver();
        for (spec, harness_type, policy_type) in SPECS {
            for known_type in [harness_type, policy_type] {
                let unknown_type = format!("Alias{suffix}");
                let imported = TestPath::new(known_type);
                let canonical =
                    TestPath::new(&format!("{}::{known_type}", spec.default_crate_name));
                let renamed = TestPath::new(&format!("renamed_{suffix}::{known_type}"));
                let aliased =
                    TestPath::new(&format!("{}::{unknown_type}", spec.default_crate_name));

                prop_assert!(!super::first_party_adapter_path_matches(imported.parsed(), spec, resolver));
                prop_assert!(super::first_party_adapter_path_matches(canonical.parsed(), spec, resolver));
                prop_assert!(!super::first_party_adapter_path_matches(renamed.parsed(), spec, resolver));
                prop_assert!(!super::first_party_adapter_path_matches(aliased.parsed(), spec, resolver));
            }
        }
    }
}
