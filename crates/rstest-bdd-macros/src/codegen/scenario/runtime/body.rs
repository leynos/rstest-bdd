//! Scenario body assembly helpers.

use proc_macro2::TokenStream as TokenStream2;
use quote::quote;

use crate::codegen::scenario::ScenarioReturnKind;

pub(super) fn build_scenario_body(
    block: &syn::Block,
    return_kind: ScenarioReturnKind,
    is_async: bool,
) -> TokenStream2 {
    match return_kind {
        ScenarioReturnKind::Unit => quote! { #block },
        ScenarioReturnKind::ResultUnit => {
            let body = if is_async {
                quote! { (async #block).await }
            } else {
                quote! { (|| #block)() }
            };
            quote! {
                let __rstest_bdd_result = #body;
                match __rstest_bdd_result {
                    Ok(()) => {}
                    Err(__rstest_bdd_err) => {
                        __rstest_bdd_scenario_guard.mark_recorded();
                        return Err(__rstest_bdd_err);
                    }
                }
                Ok(())
            }
        }
    }
}
