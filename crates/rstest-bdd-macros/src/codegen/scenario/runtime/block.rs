//! Scenario body wrapping helpers for runtime code generation.

use proc_macro2::TokenStream as TokenStream2;
use quote::quote;

use crate::return_classifier::ReturnKind;

pub(super) fn wrap_scenario_block(
    block: &syn::Block,
    is_async: bool,
    return_kind: ReturnKind,
) -> TokenStream2 {
    match return_kind {
        ReturnKind::ResultUnit | ReturnKind::ResultValue => {
            if is_async {
                quote! {
                    let __rstest_bdd_scenario_result = (async { #block }).await;
                    if __rstest_bdd_scenario_result.is_err() {
                        __rstest_bdd_scenario_guard.mark_recorded();
                    }
                    __rstest_bdd_scenario_result
                }
            } else {
                quote! {
                    let __rstest_bdd_scenario_result = (|| #block)();
                    if __rstest_bdd_scenario_result.is_err() {
                        __rstest_bdd_scenario_guard.mark_recorded();
                    }
                    __rstest_bdd_scenario_result
                }
            }
        }
        _ => quote! { #block },
    }
}
