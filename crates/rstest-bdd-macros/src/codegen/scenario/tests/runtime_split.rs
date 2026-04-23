//! Tests covering the `ScenarioConfig` execution-runtime split.

use super::{
    FeaturePath, RuntimeMode, ScenarioConfig, ScenarioName, ScenarioReturnKind, TestAttrPolicy,
    blank, generate_test_attrs,
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
