//! Helpers for wrapping scenario bodies during code generation.

use proc_macro2::TokenStream as TokenStream2;
use quote::quote;

use crate::codegen::scenario::ScenarioReturnKind;

pub(super) fn wrap_scenario_block(
    block: &syn::Block,
    return_kind: ScenarioReturnKind,
    is_async: bool,
) -> TokenStream2 {
    let block_tokens = quote! { #block };

    if !return_kind.is_fallible() {
        return block_tokens;
    }

    if is_async {
        quote! {
            match (async #block).await {
                Ok(()) => Ok(()),
                Err(__rstest_bdd_err) => {
                    __rstest_bdd_scenario_guard.mark_recorded();
                    Err(__rstest_bdd_err)
                }
            }
        }
    } else {
        quote! {
            match (|| #block)() {
                Ok(()) => Ok(()),
                Err(__rstest_bdd_err) => {
                    __rstest_bdd_scenario_guard.mark_recorded();
                    Err(__rstest_bdd_err)
                }
            }
        }
    }
}
