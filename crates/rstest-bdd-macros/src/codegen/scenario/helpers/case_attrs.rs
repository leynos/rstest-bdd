//! Case-attribute generation for Examples rows.
//!
//! Regular scenarios emit `#[case(...)]` from each row's cells. Scenario
//! outlines prepend the row index so the generated test can select the
//! matching substituted step array at runtime. Both forms share the same
//! cell-literal construction.

use proc_macro2::TokenStream as TokenStream2;
use quote::quote;

use super::{cell_to_lit, row_has_values};

/// Generate attributes for rstest cases based on examples.
pub(crate) fn generate_case_attrs(
    examples: &crate::parsing::examples::ExampleTable,
) -> Vec<TokenStream2> {
    generate_case_attrs_internal(examples, false)
}

/// Internal helper for generating case attributes with optional index prepending.
fn generate_case_attrs_internal(
    examples: &crate::parsing::examples::ExampleTable,
    prepend_index: bool,
) -> Vec<TokenStream2> {
    examples
        .rows
        .iter()
        .enumerate()
        .filter(|(_, row)| row_has_values(row))
        .map(|(idx, row)| {
            let cells = row.iter().map(|v| {
                let lit = cell_to_lit(v);
                quote! { #lit }
            });
            if prepend_index {
                let idx_lit =
                    syn::LitInt::new(&format!("{idx}usize"), proc_macro2::Span::call_site());
                quote! { #[case( #idx_lit, #(#cells),* )] }
            } else {
                quote! { #[case( #(#cells),* )] }
            }
        })
        .collect()
}

/// Generate case attributes with a prepended case index for scenario outlines.
///
/// Each `#[case(...)]` attribute includes the row index as its first argument,
/// enabling the generated test to select the correct substituted step array.
///
/// # Examples
///
/// For an Examples table with rows `["5", "apples"]` and `["10", "oranges"]`:
/// ```text
/// #[case(0usize, "5", "apples")]
/// #[case(1usize, "10", "oranges")]
/// ```
pub(crate) fn generate_indexed_case_attrs(
    examples: &crate::parsing::examples::ExampleTable,
) -> Vec<TokenStream2> {
    generate_case_attrs_internal(examples, true)
}
