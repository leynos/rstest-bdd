//! Snapshots pinning the assembled item for both scenario shapes.
//!
//! `generate_scenario_code` selects a regular or an outline path. Both must
//! emit the same item envelope and differ only in their case attributes and
//! the finalized signature. These snapshots fix the whole rendered item so a
//! change in assembly order, metadata projection, or step token shape shows up
//! as a reviewable diff.

use super::{
    FeaturePath,
    ParsedStep,
    RuntimeMode,
    ScenarioConfig,
    ScenarioName,
    ScenarioReturnKind,
    blank,
    generate_scenario_code,
};
use crate::parsing::examples::ExampleTable;

/// Build a parsed step with the supplied keyword and text.
fn step(keyword: crate::StepKeyword, text: &str) -> ParsedStep {
    ParsedStep {
        keyword,
        text: text.to_owned(),
        ..blank()
    }
}

/// Render one scenario configuration through the public codegen entry point.
fn render(config: &ScenarioConfig<'_>) -> String {
    generate_scenario_code(
        config,
        std::iter::empty(),
        std::iter::empty(),
        std::iter::empty(),
    )
    .to_string()
}

#[test]
fn regular_scenario_item_is_stable() {
    let attrs = Vec::new();
    let vis: syn::Visibility = syn::parse_quote!();
    let sig: syn::Signature = syn::parse_quote!(fn regular_snapshot());
    let block: syn::Block = syn::parse_quote!({});
    let tags = vec!["@smoke".to_owned()];
    let steps = vec![
        ParsedStep {
            docstring: Some("a note".to_owned()),
            table: Some(vec![vec!["cell".to_owned(), "value".to_owned()]]),
            ..step(crate::StepKeyword::Given, "a precondition")
        },
        step(crate::StepKeyword::Then, "an outcome"),
    ];
    let config = ScenarioConfig {
        attrs: &attrs,
        vis: &vis,
        sig: &sig,
        block: &block,
        feature_path: FeaturePath::new("tests/features/snapshot.feature".to_owned()),
        scenario_name: ScenarioName::new("regular snapshot".to_owned()),
        steps,
        examples: None,
        allow_skipped: false,
        line: 7,
        tags: &tags,
        runtime: RuntimeMode::Sync,
        attribute_runtime: RuntimeMode::Sync,
        return_kind: ScenarioReturnKind::Unit,
        harness: None,
        attributes: None,
        resolutions: None,
        fallback_diagnostics: None,
        scope: quote::quote!(::rstest_bdd::StepScope::global()),
    };

    insta::assert_snapshot!("regular_scenario_item", render(&config));
}

#[test]
fn outline_scenario_item_is_stable() {
    let attrs = Vec::new();
    let vis: syn::Visibility = syn::parse_quote!();
    let sig: syn::Signature = syn::parse_quote!(fn outline_snapshot(count: usize));
    let block: syn::Block = syn::parse_quote!({});
    let tags = Vec::new();
    let steps = vec![step(crate::StepKeyword::Given, "I have <count> items")];
    let examples = ExampleTable {
        headers: vec!["count".to_owned()],
        rows: vec![vec!["1".to_owned()], vec!["2".to_owned()]],
        row_tags: vec![Vec::new(), Vec::new()],
    };
    let config = ScenarioConfig {
        attrs: &attrs,
        vis: &vis,
        sig: &sig,
        block: &block,
        feature_path: FeaturePath::new("tests/features/snapshot.feature".to_owned()),
        scenario_name: ScenarioName::new("outline snapshot".to_owned()),
        steps,
        examples: Some(examples),
        allow_skipped: false,
        line: 11,
        tags: &tags,
        runtime: RuntimeMode::Sync,
        attribute_runtime: RuntimeMode::Sync,
        return_kind: ScenarioReturnKind::Unit,
        harness: None,
        attributes: None,
        resolutions: None,
        fallback_diagnostics: None,
        scope: quote::quote!(::rstest_bdd::StepScope::global()),
    };

    insta::assert_snapshot!("outline_scenario_item", render(&config));
}
