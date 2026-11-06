//! Item-level assembly shared by regular and outline scenario generators.
//!
//! Both shapes produce the same test item — fallback diagnostics, trait
//! assertions, test attributes, case attributes, source attributes, the
//! signature, and the body — and differ only in how the body and case
//! attributes were produced. Fixing the assembly order here stops the two
//! generators drifting apart.

use std::borrow::Cow;

use proc_macro2::TokenStream as TokenStream2;
use quote::quote;

use super::{ScenarioConfig, boundary::finalize_scenario_signature, runtime::ScenarioMetadata};

/// Project the per-scenario metadata both generators feed to the runtime.
///
/// Regular and outline scenarios differ only in how their steps are produced;
/// the metadata is the same projection of the configuration in both cases, so
/// it is built once here rather than duplicated at each call site.
pub(super) fn scenario_metadata<'a>(
    config: &'a ScenarioConfig<'_>,
    harness_api_path: Option<TokenStream2>,
) -> ScenarioMetadata<'a> {
    ScenarioMetadata {
        feature_path: &config.feature_path,
        scenario_name: &config.scenario_name,
        scenario_line: config.line,
        tags: config.tags,
        block: config.block,
        allow_skipped: config.allow_skipped,
        is_async: config.runtime.is_async(),
        return_kind: config.return_kind,
        harness: config.harness,
        harness_api_path,
        scope: config.scope.clone(),
    }
}

/// The per-shape inputs to item assembly.
///
/// Regular and outline scenarios produce the same item structure; only the
/// case attributes and the body differ. Grouping them keeps the assembler's
/// parameter list about the item it builds rather than about the shape that
/// produced it.
pub(super) struct ScenarioEmit<'a> {
    /// Harness adapter decision, when the scenario declares a harness.
    pub(super) harness_resolution: Option<&'a crate::codegen::HarnessApiResolution>,
    /// Attribute-policy decision, when the scenario declares one.
    pub(super) attributes_resolution: Option<&'a crate::codegen::HarnessApiResolution>,
    /// Attributes generated once per Examples row.
    pub(super) case_attrs: &'a [TokenStream2],
    /// The generated scenario body.
    pub(super) body: TokenStream2,
}

/// Emit the finished test item around an already-generated body.
pub(super) fn emit_scenario_test(
    config: &ScenarioConfig<'_>,
    signature: &mut Cow<'_, syn::Signature>,
    emit: ScenarioEmit<'_>,
) -> TokenStream2 {
    let ScenarioEmit {
        harness_resolution,
        attributes_resolution,
        case_attrs,
        body,
    } = emit;
    let (trait_assertions, test_attrs, underscore_expect, body) = finalize_scenario_signature(
        config,
        harness_resolution,
        attributes_resolution,
        signature,
        body,
    );
    let fallback_diagnostics = config.fallback_diagnostics;
    let attrs = config.attrs;
    let vis = config.vis;
    quote! {
        #fallback_diagnostics
        #trait_assertions
        #test_attrs
        #(#case_attrs)*
        #(#attrs)*
        #underscore_expect
        #vis #signature { #body }
    }
}
