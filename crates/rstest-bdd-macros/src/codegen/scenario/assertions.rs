//! Compile-time contract assertions emitted beside generated scenarios.

use proc_macro2::TokenStream as TokenStream2;
use quote::quote;

/// Generate sibling assertions for configured harness and attribute-policy types.
pub(super) fn generate_trait_assertions(
    harness: Option<&syn::Path>,
    attributes: Option<&syn::Path>,
) -> TokenStream2 {
    if harness.is_none() && attributes.is_none() {
        return TokenStream2::new();
    }

    let harness_assertion = harness.map(|harness_path| {
        let harness_crate = crate::codegen::rstest_bdd_harness_api_path_for(harness_path);
        quote! {
            const _: () = {
                fn __assert_harness<T: #harness_crate::HarnessAdapter + Default>() {}
                fn __call() { __assert_harness::<#harness_path>(); }
            };
        }
    });
    let attributes_assertion = attributes.map(|policy_path| {
        let harness_crate = crate::codegen::rstest_bdd_harness_api_path_for(policy_path);
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
