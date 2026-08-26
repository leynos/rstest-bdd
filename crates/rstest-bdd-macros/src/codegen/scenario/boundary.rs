//! Executable boundary for generated scenario tests.
//!
//! Resolves the ADR-008 attribute policy for a scenario and, where the
//! selected test attribute cannot consume a `Result`, rewrites the scenario
//! body so the failure still surfaces.

use std::borrow::Cow;

use proc_macro2::TokenStream as TokenStream2;
use quote::quote;

use super::{
    ScenarioConfig,
    ScenarioReturnKind,
    adapters::generate_trait_assertions,
    helpers::generate_underscore_expect,
    test_attrs::{TestAttrPolicy, generate_test_attrs_with_boundary},
};

/// Adapts a fallible scenario to GPUI's unit-returning test boundary.
///
/// Published GPUI versions may call an attributed function as a bare
/// statement, which leaves its `Result` unused. This boundary is intentionally
/// limited to generated GPUI tests; std and Tokio tests continue to return the
/// scenario result through their native `Termination` support.
pub(super) fn adapt_fallible_gpui_boundary(
    uses_gpui_boundary: bool,
    return_kind: ScenarioReturnKind,
    signature: &mut syn::Signature,
    body: TokenStream2,
) -> TokenStream2 {
    if !return_kind.is_fallible() || !uses_gpui_boundary {
        return body;
    }

    let is_async = signature.asyncness.is_some();
    signature.output = syn::ReturnType::Default;
    if is_async {
        quote! {
            match (async move { #body }).await {
                Ok(()) => {}
                Err(_) => panic!("scenario returned an error"),
            }
        }
    } else {
        quote! {
            match (|| { #body })() {
                Ok(()) => {}
                Err(_) => panic!("scenario returned an error"),
            }
        }
    }
}

/// Finalize attributes and the executable boundary for a scenario signature.
pub(super) fn finalize_scenario_signature(
    config: &ScenarioConfig<'_>,
    harness_resolution: Option<&crate::codegen::HarnessApiResolution>,
    attributes_resolution: Option<&crate::codegen::HarnessApiResolution>,
    signature: &mut Cow<'_, syn::Signature>,
    body: TokenStream2,
) -> (TokenStream2, TokenStream2, TokenStream2, TokenStream2) {
    let policy = TestAttrPolicy {
        runtime: config.attribute_runtime,
        harness: config.harness,
        attributes: config.attributes,
    };
    let generated_test_attrs =
        generate_test_attrs_with_boundary(config.attrs, &policy, config.runtime.is_async());
    let trait_assertions = generate_trait_assertions(
        config.harness.zip(harness_resolution),
        config.attributes.zip(attributes_resolution),
    );
    let adapted_body =
        if generated_test_attrs.uses_gpui_boundary && config.return_kind.is_fallible() {
            adapt_fallible_gpui_boundary(true, config.return_kind, signature.to_mut(), body)
        } else {
            body
        };
    let underscore_expect = generate_underscore_expect(signature);
    (
        trait_assertions,
        generated_test_attrs.tokens,
        underscore_expect,
        adapted_body,
    )
}
