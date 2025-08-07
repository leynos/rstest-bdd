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

/// Configuration for generating code for a single scenario test.
pub(crate) struct ScenarioConfig<'a> {
    /// Attributes on the annotated function.
    pub(crate) attrs: &'a [syn::Attribute],
    /// Visibility of the function.
    pub(crate) vis: &'a syn::Visibility,
    /// Signature of the function.
    pub(crate) sig: &'a syn::Signature,
    /// Function body.
    pub(crate) block: &'a syn::Block,
    /// Fully qualified feature file path.
    pub(crate) feature_path: String,
    /// Name of the scenario.
    pub(crate) scenario_name: String,
    /// Steps in the scenario.
    pub(crate) steps: Vec<crate::parsing::feature::ParsedStep>,
    /// Examples table for scenario outlines.
    pub(crate) examples: Option<crate::parsing::examples::ExampleTable>,
}

/// Generate the runtime test for a single scenario.
pub(crate) fn generate_scenario_code(
    config: ScenarioConfig<'_>,
    ctx_inserts: impl Iterator<Item = TokenStream2>,
) -> TokenStream {
    let ScenarioConfig {
        attrs,
        vis,
        sig,
        block,
        feature_path: feature_path_str,
        scenario_name,
        steps,
        examples,
    } = config;

    let keywords: Vec<_> = steps.iter().map(|s| keyword_to_token(s.keyword)).collect();
    let values = steps.iter().map(|s| &s.text);
    let tables: Vec<_> = steps
        .iter()
        .map(|s| {
            s.table.as_ref().map_or_else(
                || quote! { None },
                |rows| {
                    let row_tokens = rows.iter().map(|row| {
                        let cells = row.iter().map(|cell| {
                            let lit = syn::LitStr::new(cell, proc_macro2::Span::call_site());
                            quote! { #lit }
                        });
                        quote! { &[#(#cells),*][..] }
                    });
                    quote! { Some(&[#(#row_tokens),*][..]) }
                },
            )
        })
        .collect();

    let case_attrs = examples.map_or_else(Vec::new, |ex| generate_case_attrs(&ex));

    TokenStream::from(quote! {
        #[rstest::rstest]
        #(#case_attrs)*
        #(#attrs)*
        #vis #sig {
            let steps = [#((#keywords, #values, #tables)),*];
            let mut ctx = rstest_bdd::StepContext::default();
            #(#ctx_inserts)*
            for (index, (keyword, text, table)) in steps.iter().enumerate() {
                if let Some(f) = rstest_bdd::find_step(*keyword, (*text).into()) {
                    if let Err(err) = f(&ctx, text, *table) {
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
