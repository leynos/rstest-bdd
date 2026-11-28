//! Tests pinning how many fallback diagnostics one expansion boundary emits.
//!
//! `scenarios!` generates a test per discovered scenario, so the contract is
//! one diagnostic per distinct qualifying supplied path rather than one per
//! generated test. These tests exercise the two halves of that contract: the
//! boundary emits, and scenarios generated beneath it do not.
//!
//! The marker is the stable `#[deprecated]` item, so the counts below only
//! apply on stable. Nightly routes the same guidance through
//! `proc_macro::Diagnostic` and leaves the token stream free of markers.

use super::{
    FeaturePath,
    RuntimeMode,
    ScenarioConfig,
    ScenarioName,
    ScenarioReturnKind,
    blank,
    generate_scenario_code,
};
use crate::codegen::SharedPolicyDiagnostics;

// Every diagnostic marker shares this prefix, so the counts below stay
// correct as codes gain their own headline names.
const MARKER: &str = "struct RstestBdd";

/// A path reaching the adapter crate through a local re-export, so first-party
/// detection cannot match it and the fallback qualifies.
fn aliased_harness() -> syn::Path {
    syn::parse_quote!(alias::rstest_bdd_harness_tokio::TokioHarness)
}

// Only the stable marker-counting tests need these, and `rstest_bdd_nightly`
// compiles those out. Gating the helpers identically keeps the nightly build
// free of dead code without suppressing the lint.
#[cfg(not(rstest_bdd_nightly))]
fn aliased_attributes() -> syn::Path {
    syn::parse_quote!(alias::rstest_bdd_harness_tokio::TokioAttributePolicy)
}

#[cfg(not(rstest_bdd_nightly))]
fn marker_count(tokens: &proc_macro2::TokenStream) -> usize {
    tokens.to_string().matches(MARKER).count()
}

/// Generate one scenario against a resolution an enclosing boundary owns.
fn scenario_output_with_shared_resolutions(
    harness: Option<&syn::Path>,
    resolutions: &SharedPolicyDiagnostics,
) -> String {
    let attrs = Vec::new();
    let vis: syn::Visibility = syn::parse_quote!();
    let sig: syn::Signature = syn::parse_quote!(fn shared_resolution_scenario());
    let block: syn::Block = syn::parse_quote!({});
    let tags = Vec::new();
    let config = ScenarioConfig {
        attrs: &attrs,
        vis: &vis,
        sig: &sig,
        block: &block,
        feature_path: FeaturePath::new("tests/features/shared.feature".to_owned()),
        scenario_name: ScenarioName::new("shared resolution".to_owned()),
        steps: vec![blank()],
        examples: None,
        allow_skipped: false,
        line: 1,
        tags: &tags,
        runtime: RuntimeMode::Sync,
        attribute_runtime: RuntimeMode::Sync,
        return_kind: ScenarioReturnKind::Unit,
        harness,
        attributes: None,
        resolutions: Some(resolutions),
        fallback_diagnostics: None,
        scope: quote::quote!(::rstest_bdd::StepScope::global()),
    };

    generate_scenario_code(
        &config,
        std::iter::empty(),
        std::iter::empty(),
        std::iter::empty(),
    )
    .to_string()
}

#[cfg(not(rstest_bdd_nightly))]
#[test]
fn boundary_emits_one_diagnostic_per_qualifying_path() {
    let harness = aliased_harness();
    let resolutions = SharedPolicyDiagnostics::for_scenario(crate::codegen::ScenarioRequest {
        harness: Some(&harness),
        attributes: None,
        facts: scenario_facts(),
    });

    assert_eq!(
        marker_count(&resolutions.warning_tokens()),
        1,
        "one supplied path must yield one diagnostic"
    );
}

#[cfg(not(rstest_bdd_nightly))]
#[test]
fn boundary_emits_two_diagnostics_for_combined_harness_and_attributes() {
    let harness = aliased_harness();
    let attributes = aliased_attributes();
    let resolutions = SharedPolicyDiagnostics::for_scenario(crate::codegen::ScenarioRequest {
        harness: Some(&harness),
        attributes: Some(&attributes),
        facts: scenario_facts(),
    });

    assert_eq!(
        marker_count(&resolutions.warning_tokens()),
        2,
        "harness and attributes are independent supplied paths"
    );
}

/// Scenarios generated beneath a boundary must not repeat its diagnostic,
/// which is what keeps a multi-scenario `scenarios!` expansion at one.
#[test]
fn scenario_generated_from_shared_resolution_emits_no_diagnostic() {
    let harness = aliased_harness();
    let resolutions = SharedPolicyDiagnostics::for_scenario(crate::codegen::ScenarioRequest {
        harness: Some(&harness),
        attributes: None,
        facts: scenario_facts(),
    });

    let output = scenario_output_with_shared_resolutions(Some(&harness), &resolutions);

    assert_eq!(
        output.matches(MARKER).count(),
        0,
        "the enclosing boundary already emitted this diagnostic: {output}"
    );
}

/// The three-marker witness: a non-canonical harness and a non-canonical,
/// conflicting policy.
///
/// W002 cannot co-occur with W003 (it requires no policy), so this is the only
/// shape that renders three warnings at one boundary (decision D32).
#[cfg(not(rstest_bdd_nightly))]
#[test]
fn boundary_emits_three_diagnostics_for_evidence_and_conflict() {
    let harness: syn::Path = syn::parse_quote!(alias::rstest_bdd_harness_tokio::TokioHarness);
    let attributes: syn::Path =
        syn::parse_quote!(alias::rstest_bdd_harness_gpui::GpuiAttributePolicy);
    let resolutions = SharedPolicyDiagnostics::for_scenario(crate::codegen::ScenarioRequest {
        harness: Some(&harness),
        attributes: Some(&attributes),
        facts: scenario_facts(),
    });

    assert_eq!(
        marker_count(&resolutions.warning_tokens()),
        3,
        "two evidenced slots and a framework conflict render three warnings"
    );
}

/// A canonical crate-root path is not a fallback, so nothing is emitted.
#[cfg(not(rstest_bdd_nightly))]
#[test]
fn canonical_path_yields_no_diagnostic() {
    let harness: syn::Path = syn::parse_quote!(rstest_bdd_harness_tokio::TokioHarness);
    let resolutions = SharedPolicyDiagnostics::for_scenario(crate::codegen::ScenarioRequest {
        harness: Some(&harness),
        attributes: None,
        facts: scenario_facts(),
    });

    assert_eq!(
        marker_count(&resolutions.warning_tokens()),
        0,
        "canonical first-party paths must stay diagnostic-free"
    );
}

/// Diagnostic facts for a test scenario with no user attributes.
fn scenario_facts() -> crate::codegen::ScenarioFacts {
    crate::codegen::ScenarioFacts {
        is_async: false,
        has_executor_attr: false,
        has_gpui_test_attr: false,
    }
}

/// A resolver that reports its crates as absent, with no manifest failure.
#[cfg(not(rstest_bdd_nightly))]
struct Unresolvable;

#[cfg(not(rstest_bdd_nightly))]
impl crate::codegen::CrateResolver for Unresolvable {
    fn resolved_root(
        &self,
        _: &str,
    ) -> Result<Option<proc_macro2::Ident>, crate::codegen::ResolveError> {
        Ok(None)
    }

    fn is_direct_dependency(&self, _: &str) -> Result<bool, crate::codegen::ResolveError> {
        Ok(false)
    }
}

/// The boundary accepts an injected resolver, so verification can exercise the
/// production wiring instead of the unit-test stub.
#[cfg(not(rstest_bdd_nightly))]
#[test]
fn boundary_with_an_injected_resolver_reports_no_evidence() {
    static RESOLVER: Unresolvable = Unresolvable;
    let harness: syn::Path = syn::parse_quote!(alias::rstest_bdd_harness_tokio::TokioHarness);
    let resolutions = SharedPolicyDiagnostics::for_scenario_with(
        crate::codegen::ScenarioRequest {
            harness: Some(&harness),
            attributes: None,
            facts: scenario_facts(),
        },
        &RESOLVER,
    );

    assert_eq!(
        marker_count(&resolutions.warning_tokens()),
        0,
        "an unresolvable adapter crate carries no first-party evidence"
    );
}
