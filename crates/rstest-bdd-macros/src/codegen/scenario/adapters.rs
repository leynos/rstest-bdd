//! Adapter API resolution and trait assertions for one generated scenario.
//!
//! Keeping resolution separate from token assembly makes the emission contract
//! explicit: whichever expansion boundary owns the supplied paths resolves them
//! and emits their diagnostics, and every scenario generated beneath that
//! boundary reuses the decision without re-emitting.

use proc_macro2::TokenStream as TokenStream2;
use quote::quote;
use std::borrow::Cow;

use super::ScenarioConfig;
use crate::codegen::{HarnessApiResolution, SharedAdapterResolutions};

/// The adapter decision for one scenario plus the diagnostics it owns.
pub(super) struct ScenarioAdapters<'a> {
    pub(super) resolutions: Cow<'a, SharedAdapterResolutions>,
    /// Fallback diagnostic tokens this expansion must splice into its output.
    ///
    /// Empty when an enclosing boundary already emitted them, which is what
    /// holds `scenarios!` to one diagnostic per supplied path rather than one
    /// per generated scenario.
    pub(super) diagnostics: TokenStream2,
}

/// Select the adapter API paths this expansion should generate against.
///
/// Reuses the enclosing boundary's decision when one was supplied, otherwise
/// resolves the supplied paths here and takes ownership of their diagnostics.
pub(super) fn resolve_scenario_adapters<'a>(
    config: &'a ScenarioConfig<'_>,
) -> ScenarioAdapters<'a> {
    config.resolutions.map_or_else(
        || {
            let resolutions = SharedAdapterResolutions::resolve(config.harness, config.attributes);
            // Owning the resolution means owning its emission: this is the
            // only place the diagnostic fires for a `#[scenario]` expansion.
            let diagnostics = resolutions.emit_diagnostics();
            ScenarioAdapters {
                resolutions: Cow::Owned(resolutions),
                diagnostics,
            }
        },
        |shared| ScenarioAdapters {
            resolutions: Cow::Borrowed(shared),
            diagnostics: TokenStream2::new(),
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
