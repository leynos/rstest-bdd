//! Code generation for scenario tests.

use super::keyword_to_token;
use proc_macro::TokenStream;
use proc_macro2::TokenStream as TokenStream2;
use quote::quote;

/// Create a `LitStr` from an examples table cell.
fn cell_to_lit(value: &str) -> syn::LitStr {
    syn::LitStr::new(value, proc_macro2::Span::call_site())
}

/// Generate attributes for rstest cases based on examples.
pub(crate) fn generate_case_attrs(
    examples: &crate::parsing::examples::ExampleTable,
) -> Vec<TokenStream2> {
    examples
        .rows
        .iter()
        .filter(|row| row.iter().any(|cell| !cell.is_empty()))
        .map(|row| {
            let cells = row.iter().map(|v| {
                let lit = cell_to_lit(v);
                quote! { #lit }
            });
            quote! { #[case( #(#cells),* )] }
        })
        .collect()
}

#[expect(
    clippy::too_many_arguments,
    clippy::needless_pass_by_value,
    reason = "signature defined by requirements"
)]
pub(crate) fn generate_scenario_code(
    attrs: &[syn::Attribute],
    vis: &syn::Visibility,
    sig: &syn::Signature,
    block: &syn::Block,
    feature_path_str: String,
    scenario_name: String,
    steps: Vec<(rstest_bdd::StepKeyword, String)>,
    examples: Option<crate::parsing::examples::ExampleTable>,
    ctx_inserts: impl Iterator<Item = TokenStream2>,
) -> TokenStream {
    let keywords: Vec<_> = steps.iter().map(|(k, _)| keyword_to_token(*k)).collect();
    let values = steps.iter().map(|(_, v)| v);

    let case_attrs = examples.map_or_else(Vec::new, |ex| generate_case_attrs(&ex));

    TokenStream::from(quote! {
        #[rstest::rstest]
        #(#case_attrs)*
        #(#attrs)*
        #vis #sig {
            let steps = [#((#keywords, #values)),*];
            let mut ctx = rstest_bdd::StepContext::default();
            #(#ctx_inserts)*
            for (index, (keyword, text)) in steps.iter().enumerate() {
                if let Some(f) = rstest_bdd::find_step(*keyword, (*text).into()) {
                    if let Err(err) = f(&ctx, text) {
                        panic!(
                            "Step failed at index {}: {} {} - {}\n(feature: {}, scenario: {})",
                            index,
                            keyword.as_str(),
                            text,
                            err,
                            #feature_path_str,
                            #scenario_name
                        );
                    }
                } else {
                    panic!(
                        "Step not found at index {}: {} {} (feature: {}, scenario: {})",
                        index,
                        keyword.as_str(),
                        text,
                        #feature_path_str,
                        #scenario_name
                    );
                }
            }
            #block
        }
    })
}
