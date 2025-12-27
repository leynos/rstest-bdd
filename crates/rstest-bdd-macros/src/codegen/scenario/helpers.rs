//! Helpers for generating scenario code from parsed examples and steps.

use proc_macro2::TokenStream as TokenStream2;
use quote::{ToTokens, quote};

use crate::parsing::placeholder::substitute_placeholders;

/// Result type for substituted step content: (text, docstring, table).
type SubstitutedStepContent = (String, Option<String>, Option<Vec<Vec<String>>>);

/// Result type for processed step tokens: (keywords, values, docstrings, tables).
pub(crate) type ProcessedStepTokens = (
    Vec<TokenStream2>,
    Vec<TokenStream2>,
    Vec<TokenStream2>,
    Vec<TokenStream2>,
);

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
    examples
        .rows
        .iter()
        .enumerate()
        .filter(|(_, row)| row.iter().any(|cell| !cell.is_empty()))
        .map(|(idx, row)| {
            let idx_lit = syn::LitInt::new(&format!("{idx}usize"), proc_macro2::Span::call_site());
            let cells = row.iter().map(|v| {
                let lit = cell_to_lit(v);
                quote! { #lit }
            });
            quote! { #[case( #idx_lit, #(#cells),* )] }
        })
        .collect()
}

/// Substitutes placeholders in step text, docstring, and table cells.
///
/// Returns the substituted text, docstring, and table for a single step given
/// the Examples table headers and a specific row's values.
fn substitute_step_content(
    text: &str,
    docstring: Option<&String>,
    table: Option<&[Vec<String>]>,
    headers: &[String],
    row: &[String],
) -> Result<SubstitutedStepContent, crate::parsing::placeholder::PlaceholderError> {
    let substituted_text = substitute_placeholders(text, headers, row)?;

    let substituted_docstring = docstring
        .map(|d| substitute_placeholders(d, headers, row))
        .transpose()?;

    let substituted_table = table
        .map(|t| {
            t.iter()
                .map(|table_row| {
                    table_row
                        .iter()
                        .map(|cell| substitute_placeholders(cell, headers, row))
                        .collect::<Result<Vec<_>, _>>()
                })
                .collect::<Result<Vec<_>, _>>()
        })
        .transpose()?;

    Ok((substituted_text, substituted_docstring, substituted_table))
}

/// Process steps with placeholder substitution for one Examples row.
///
/// Substitutes `<placeholder>` tokens in step text, docstrings, and table cells
/// with values from the given Examples row, then converts to token streams.
///
/// # Arguments
///
/// * `steps` - The parsed steps from the scenario
/// * `headers` - Column headers from the Examples table
/// * `row` - Values for the current row, aligned with headers
///
/// # Returns
///
/// A tuple of token stream vectors for keywords, values, docstrings, and tables
/// with all placeholders substituted.
pub(crate) fn process_steps_substituted(
    steps: &[crate::parsing::feature::ParsedStep],
    headers: &[String],
    row: &[String],
) -> Result<ProcessedStepTokens, proc_macro2::TokenStream> {
    // Resolve textual conjunctions (And/But) to the previous primary keyword
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

    let mut values = Vec::with_capacity(steps.len());
    let mut docstrings = Vec::with_capacity(steps.len());
    let mut tables = Vec::with_capacity(steps.len());

    for step in steps {
        let (sub_text, sub_doc, sub_table) = substitute_step_content(
            &step.text,
            step.docstring.as_ref(),
            step.table.as_deref(),
            headers,
            row,
        )
        .map_err(|e| {
            let err = syn::Error::new(proc_macro2::Span::call_site(), e.to_string());
            err.into_compile_error()
        })?;

        let text_lit = cell_to_lit(&sub_text);
        values.push(quote! { #text_lit });

        let doc_tokens = sub_doc.map_or_else(
            || quote! { None },
            |d| {
                let lit = syn::LitStr::new(&d, proc_macro2::Span::call_site());
                quote! { Some(#lit) }
            },
        );
        docstrings.push(doc_tokens);

        tables.push(generate_table_tokens(sub_table.as_deref()));
    }

    Ok((keyword_tokens, values, docstrings, tables))
}
