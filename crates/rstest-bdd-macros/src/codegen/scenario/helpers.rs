//! Helpers for generating scenario code from parsed examples and steps.
use proc_macro2::TokenStream as TokenStream2;
use quote::{ToTokens, quote};

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

fn generate_table_tokens(table: Option<&[Vec<String>]>) -> TokenStream2 {
    table.map_or_else(
        || quote! { None },
        |rows| {
            if rows.is_empty() {
                // Explicitly type the empty slice to avoid inference pitfalls when no rows exist.
                quote! { Some(&[] as &[&[&str]]) }
            } else {
                let row_tokens = rows.iter().map(|row| {
                    let cells = row.iter().map(|cell| {
                        let lit = cell_to_lit(cell);
                        quote! { #lit }
                    });
                    quote! { &[#(#cells),*][..] }
                });
                quote! { Some(&[#(#row_tokens),*][..]) }
            }
        },
    )
}

/// Process parsed steps into tokens for keywords, values, and tables.
///
/// # Examples
/// ```rust,ignore
/// use crate::StepKeyword;
/// use crate::parsing::feature::ParsedStep;
/// // Note: `span` is available only with the `compile-time-validation` feature.
/// let steps = vec![ParsedStep {
///     keyword: StepKeyword::Given,
///     text: "x".into(),
///     docstring: None,
///     table: None,
///     span: proc_macro2::Span::call_site(),
/// }];
/// let (k, v, t) = process_steps(&steps);
/// assert_eq!(v.len(), 1);
/// ```
pub(crate) fn process_steps(
    steps: &[crate::parsing::feature::ParsedStep],
) -> (
    Vec<TokenStream2>,
    Vec<TokenStream2>,
    Vec<TokenStream2>,
    Vec<TokenStream2>,
) {
    // Resolve textual conjunctions (And/But) to the previous primary keyword
    // without depending on the validation module, which is behind an optional
    // feature. We seed with the first primary keyword or Given by default.
    let keyword_tokens = {
        let mut prev = steps
            .iter()
            .find_map(|s| match s.keyword {
                crate::StepKeyword::And | crate::StepKeyword::But => None,
                other => Some(other),
            })
            .or(Some(crate::StepKeyword::Given));
        steps.iter().map(move |s| s.keyword.resolve(&mut prev))
    }
    .map(|kw| kw.to_token_stream())
    .collect::<Vec<_>>();
    debug_assert_eq!(keyword_tokens.len(), steps.len());
    let values = steps
        .iter()
        .map(|s| {
            let lit = cell_to_lit(&s.text);
            quote! { #lit }
        })
        .collect();
    let docstrings = steps
        .iter()
        .map(|s| {
            s.docstring.as_ref().map_or_else(
                || quote! { None },
                |d| {
                    let lit = syn::LitStr::new(d, proc_macro2::Span::call_site());
                    quote! { Some(#lit) }
                },
            )
        })
        .collect();
    let tables = steps
        .iter()
        .map(|s| generate_table_tokens(s.table.as_deref()))
        .collect();
    (keyword_tokens, values, docstrings, tables)
}
