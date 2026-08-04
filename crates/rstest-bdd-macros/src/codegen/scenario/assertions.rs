//! Compile-time contract assertions emitted beside generated scenarios.

use proc_macro2::TokenStream as TokenStream2;
use quote::quote;

/// Generate sibling assertions for configured harness and attribute-policy types.
pub(super) fn generate_trait_assertions(
    harness: Option<(&syn::Path, &crate::codegen::HarnessApiResolution)>,
    attributes: Option<(&syn::Path, &crate::codegen::HarnessApiResolution)>,
) -> TokenStream2 {
    if harness.is_none() && attributes.is_none() {
        return TokenStream2::new();
    }

    let harness_assertion = harness.map(|(harness_path, resolution)| {
        let harness_crate = &resolution.api_path;
        let fallback_diagnostic = crate::codegen::first_party_adapter_fallback_diagnostic(resolution);
        quote! {
            #fallback_diagnostic
            const _: () = {
                fn __assert_harness<T: #harness_crate::HarnessAdapter + Default>() {}
                fn __call() { __assert_harness::<#harness_path>(); }
            };
        }
    });
    let attributes_assertion = attributes.map(|(policy_path, resolution)| {
        let harness_crate = &resolution.api_path;
        let fallback_diagnostic = crate::codegen::first_party_adapter_fallback_diagnostic(resolution);
        quote! {
            #fallback_diagnostic
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
