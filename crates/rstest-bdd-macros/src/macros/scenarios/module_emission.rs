//! Emits the module that contains generated `scenarios!` tests.

use proc_macro2::TokenStream as TokenStream2;
use quote::{format_ident, quote};

use super::{GeneratedScenariosModule, sanitize_ident};

/// Emit the generated scenarios module after expansion preparation completes.
pub(super) fn generate_scenarios_module(module: GeneratedScenariosModule) -> TokenStream2 {
    let GeneratedScenariosModule {
        dir,
        dir_lit,
        feature_paths,
        fallback_diagnostics,
        tests,
        errors,
    } = module;
    // Emit one rebuild dependency per discovered file, even when tag
    // filtering prevents that file from generating a scenario test.
    let tracking_items = feature_paths
        .iter()
        .map(|path| crate::codegen::tracking::feature_tracking_item(path, dir_lit.span()))
        .collect::<Vec<_>>();
    let module_ident = {
        let base = dir
            .file_name()
            .and_then(|segment| segment.to_str())
            .unwrap_or("scenarios");
        format_ident!("{}_scenarios", sanitize_ident(base))
    };
    let module_doc = format!("Scenarios auto-generated from `{}`.", dir_lit.value());

    quote! {
        #(#tracking_items)*
        #[doc = #module_doc]
        mod #module_ident {
            use super::*;
            #fallback_diagnostics
            #(#tests)*
            #(#errors)*
        }
    }
}
