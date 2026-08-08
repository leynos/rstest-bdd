//! Tests covering the `ScenarioConfig` execution-runtime split.

use super::{
    FeaturePath, RuntimeMode, ScenarioConfig, ScenarioName, ScenarioReturnKind, TestAttrPolicy,
    blank, generate_scenario_code, generate_test_attrs,
};

#[test]
fn scenario_config_keeps_attribute_runtime_separate_from_execution_runtime() {
    let attrs: Vec<syn::Attribute> = Vec::new();
    let vis: syn::Visibility = syn::parse_quote!();
    let sig: syn::Signature = syn::parse_quote!(async fn split_runtime_test());
    let block: syn::Block = syn::parse_quote!({});
    let tags: Vec<String> = Vec::new();
    let config = ScenarioConfig {
        attrs: &attrs,
        vis: &vis,
        sig: &sig,
        block: &block,
        feature_path: FeaturePath::new("tests/features/runtime_split.feature".to_owned()),
        scenario_name: ScenarioName::new("attribute runtime split".to_owned()),
        steps: vec![blank()],
        examples: None,
        allow_skipped: false,
        line: 1,
        tags: &tags,
        runtime: RuntimeMode::TokioCurrentThread,
        attribute_runtime: RuntimeMode::Sync,
        return_kind: ScenarioReturnKind::Unit,
        harness: None,
        attributes: None,
        resolutions: None,
    };

    let attrs = generate_test_attrs(
        config.attrs,
        &TestAttrPolicy {
            runtime: config.attribute_runtime,
            harness: config.harness,
            attributes: config.attributes,
        },
        config.runtime.is_async(),
    );
    let output = attrs.to_string();

    assert!(
        config.runtime.is_async(),
        "expected execution runtime to keep async generation enabled"
    );
    assert!(
        output.contains("rstest :: rstest"),
        "expected generated attributes to include rstest::rstest, got: {output}"
    );
    assert!(
        !output.contains("tokio :: test"),
        "expected generated attributes to follow attribute_runtime instead of execution runtime, got: {output}"
    );
}

/// Generate scenario code for a harness path that reaches the adapter crate
/// through a local re-export, so first-party detection falls back.
fn aliased_harness_scenario_output() -> String {
    let attrs = Vec::new();
    let vis: syn::Visibility = syn::parse_quote!();
    let sig: syn::Signature = syn::parse_quote!(fn aliased_harness());
    let block: syn::Block = syn::parse_quote!({});
    let tags = Vec::new();
    let harness: syn::Path = syn::parse_quote!(alias::rstest_bdd_harness_tokio::TokioHarness);
    let config = ScenarioConfig {
        attrs: &attrs,
        vis: &vis,
        sig: &sig,
        block: &block,
        feature_path: FeaturePath::new("tests/features/aliased.feature".to_owned()),
        scenario_name: ScenarioName::new("aliased harness".to_owned()),
        steps: vec![blank()],
        examples: None,
        allow_skipped: false,
        line: 1,
        tags: &tags,
        runtime: RuntimeMode::Sync,
        attribute_runtime: RuntimeMode::Sync,
        return_kind: ScenarioReturnKind::Unit,
        harness: Some(&harness),
        attributes: None,
        resolutions: None,
    };

    generate_scenario_code(
        &config,
        std::iter::empty(),
        std::iter::empty(),
        std::iter::empty(),
    )
    .to_string()
}

/// Stable toolchains cannot emit procedural-macro warnings, so the fallback
/// guidance rides along as a generated `#[deprecated]` marker item.
#[cfg(not(rstest_bdd_nightly))]
#[test]
fn aliased_harness_scenario_emits_one_stable_fallback_diagnostic() {
    let output = aliased_harness_scenario_output();

    assert_eq!(
        output
            .matches("struct RstestBddFirstPartyAdapterFallback")
            .count(),
        1,
        "fallback diagnostic marker should be emitted once: {output}"
    );
}

/// Nightly routes the same guidance through `proc_macro::Diagnostic`, so the
/// generated tokens must stay free of the stable marker. Asserting its absence
/// keeps the two emission paths mutually exclusive rather than additive.
#[cfg(rstest_bdd_nightly)]
#[test]
fn aliased_harness_scenario_omits_stable_fallback_marker_on_nightly() {
    let output = aliased_harness_scenario_output();

    assert_eq!(
        output
            .matches("struct RstestBddFirstPartyAdapterFallback")
            .count(),
        0,
        "nightly emits a native diagnostic instead of the marker: {output}"
    );
}
