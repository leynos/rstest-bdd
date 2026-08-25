//! Adapter API resolution and trait assertions for one generated scenario.
//!
//! Keeping resolution separate from token assembly makes the emission contract
//! explicit: whichever expansion boundary owns the supplied paths resolves them
//! and emits their diagnostics, and every scenario generated beneath that
//! boundary reuses the decision without re-emitting.

use std::borrow::Cow;

use proc_macro2::TokenStream as TokenStream2;
use quote::quote;

use super::ScenarioConfig;
use crate::codegen::{HarnessApiResolution, SharedAdapterResolutions};

/// The adapter decision for one scenario.
pub(super) struct ScenarioAdapters<'a> {
    pub(super) resolutions: Cow<'a, SharedAdapterResolutions>,
}

/// Select the adapter API paths this expansion should generate against.
///
/// Reuses the enclosing boundary's decision when one was supplied, otherwise
/// resolves the supplied paths locally. This query remains side-effect free:
/// macro-expansion boundaries emit diagnostics before passing their tokens to
/// scenario code generation.
pub(super) fn resolve_scenario_adapters<'a>(
    config: &'a ScenarioConfig<'_>,
) -> ScenarioAdapters<'a> {
    config.resolutions.map_or_else(
        || ScenarioAdapters {
            resolutions: Cow::Owned(SharedAdapterResolutions::resolve(
                config.harness,
                config.attributes,
            )),
        },
        |shared| ScenarioAdapters {
            resolutions: Cow::Borrowed(shared),
        },
    )
}

/// Generate compile-time trait-bound const assertions for harness and attribute
/// policy types. These are emitted as sibling items alongside the test function
/// so they produce clear compiler errors when a type does not implement the
/// required trait.
pub(super) fn generate_trait_assertions(
    harness: Option<(&syn::Path, &HarnessApiResolution)>,
    attributes: Option<(&syn::Path, &HarnessApiResolution)>,
) -> TokenStream2 {
    if harness.is_none() && attributes.is_none() {
        return TokenStream2::new();
    }

    let harness_assertion = harness.map(|(harness_path, resolution)| {
        let harness_crate = &resolution.api_path;
        quote! {
            const _: () = {
                fn __assert_harness<T: #harness_crate::HarnessAdapter + Default>() {}
                fn __call() { __assert_harness::<#harness_path>(); }
            };
        }
    });
    let attributes_assertion = attributes.map(|(policy_path, resolution)| {
        let harness_crate = &resolution.api_path;
        quote! {
            const _: () = {
                fn __assert_attr_policy<T: #harness_crate::AttributePolicy>() {}
                fn __call() { __assert_attr_policy::<#policy_path>(); }
            };
        }
    });

    quote! {
        #harness_assertion
        #attributes_assertion
    }
}
