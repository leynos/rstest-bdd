//! Helpers for generating scenario code from parsed examples and steps.

use proc_macro2::TokenStream as TokenStream2;
use quote::{ToTokens, quote};

use super::domain::{Docstring, ExampleHeaders, ExampleRow, StepText};
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

/// Context for placeholder substitution containing headers and row values.
pub(crate) struct SubstitutionContext<'a> {
    /// Column headers from the Examples table.
    pub(crate) headers: &'a ExampleHeaders,
    /// Row values aligned with headers.
    pub(crate) row: &'a ExampleRow,
}

impl<'a> SubstitutionContext<'a> {
    /// Creates a new substitution context.
    pub(crate) fn new(headers: &'a ExampleHeaders, row: &'a ExampleRow) -> Self {
        Self { headers, row }
    }
}

/// Create a `LitStr` from an examples table cell.
fn cell_to_lit(value: &str) -> syn::LitStr {
    syn::LitStr::new(value, proc_macro2::Span::call_site())
}

/// Returns true when an Examples row contains at least one non-empty cell.
pub(crate) fn row_has_values(row: &[String]) -> bool {
    row.iter().any(|cell| !cell.is_empty())
}

/// Returns true when any parameter uses an underscore-prefixed identifier.
///
/// The single underscore `_` pattern is ignored. Only identifier patterns are
/// considered; non-identifier patterns are ignored.
pub(crate) fn has_underscore_prefixed_params(sig: &syn::Signature) -> bool {
    sig.inputs.iter().any(|arg| {
        let syn::FnArg::Typed(pat_type) = arg else {
            return false;
        };
        let syn::Pat::Ident(pat_ident) = pat_type.pat.as_ref() else {
            return false;
        };
        let name = pat_ident.ident.to_string();
        name.starts_with('_') && name.len() > 1
    })
}

/// Emit a lint suppression attribute when underscore-prefixed parameters exist.
pub(crate) fn generate_underscore_expect(sig: &syn::Signature) -> TokenStream2 {
    if has_underscore_prefixed_params(sig) {
        quote! {
            #[expect(
                clippy::used_underscore_binding,
                reason = "rstest-bdd scenario parameters are used by generated code"
            )]
        }
    } else {
        quote! {}
    }
}

fn resolve_keyword_tokens(steps: &[crate::parsing::feature::ParsedStep]) -> Vec<TokenStream2> {
    // Resolve textual conjunctions (And/But) to the previous primary keyword
    // without depending on the validation module, which is behind an optional
    // feature. We seed with the first primary keyword or Given by default.
    let mut prev = steps
        .iter()
        .find_map(|s| match s.keyword {
            crate::StepKeyword::And | crate::StepKeyword::But => None,
            other => Some(other),
        })
        .or(Some(crate::StepKeyword::Given));
    steps
        .iter()
        .map(move |s| s.keyword.resolve(&mut prev))
        .map(|kw| kw.to_token_stream())
        .collect()
}

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
    let keyword_tokens = resolve_keyword_tokens(steps);
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
    generate_case_attrs_internal(examples, true)
}

fn substitute_table_placeholders(
    table: &[Vec<String>],
    context: &SubstitutionContext<'_>,
) -> Result<Vec<Vec<String>>, crate::parsing::placeholder::PlaceholderError> {
    table
        .iter()
        .map(|table_row| {
            table_row
                .iter()
                .map(|cell| {
                    substitute_placeholders(
                        cell,
                        context.headers.as_slice(),
                        context.row.as_slice(),
                    )
                })
                .collect::<Result<Vec<_>, _>>()
        })
        .collect::<Result<Vec<_>, _>>()
}

/// Substitutes placeholders in step text, docstring, and table cells.
///
/// Returns the substituted text, docstring, and table for a single step given
/// the substitution context containing Examples table headers and row values.
fn substitute_step_content(
    text: &StepText,
    docstring: Option<&Docstring>,
    table: Option<&[Vec<String>]>,
    context: &SubstitutionContext<'_>,
) -> Result<SubstitutedStepContent, crate::parsing::placeholder::PlaceholderError> {
    let substituted_text = substitute_placeholders(
        text.as_str(),
        context.headers.as_slice(),
        context.row.as_slice(),
    )?;

    let substituted_docstring = docstring
        .map(|d| {
            substitute_placeholders(
                d.as_str(),
                context.headers.as_slice(),
                context.row.as_slice(),
            )
        })
        .transpose()?;

    let substituted_table = table
        .map(|t| substitute_table_placeholders(t, context))
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
    headers: &ExampleHeaders,
    row: &ExampleRow,
) -> Result<ProcessedStepTokens, proc_macro2::TokenStream> {
    let context = SubstitutionContext::new(headers, row);

    let keyword_tokens = resolve_keyword_tokens(steps);

    let mut values = Vec::with_capacity(steps.len());
    let mut docstrings = Vec::with_capacity(steps.len());
    let mut tables = Vec::with_capacity(steps.len());

    for step in steps {
        let step_text = StepText::new(step.text.clone());
        let step_docstring = step
            .docstring
            .as_ref()
            .map(|doc| Docstring::new(doc.clone()));
        let (sub_text, sub_doc, sub_table) = substitute_step_content(
            &step_text,
            step_docstring.as_ref(),
            step.table.as_deref(),
            &context,
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

#[cfg(test)]
mod tests {
    use super::*;
    use syn::parse_quote;

    #[test]
    fn detects_underscore_prefixed_param() {
        let sig: syn::Signature = parse_quote! { fn test(_state: State) };
        assert!(has_underscore_prefixed_params(&sig));
    }

    #[test]
    fn ignores_single_underscore() {
        let sig: syn::Signature = parse_quote! { fn test(_: State) };
        assert!(!has_underscore_prefixed_params(&sig));
    }

    #[test]
    fn ignores_non_underscore_params() {
        let sig: syn::Signature = parse_quote! { fn test(state: State) };
        assert!(!has_underscore_prefixed_params(&sig));
    }

    #[test]
    fn detects_mixed_params() {
        let sig: syn::Signature = parse_quote! { fn test(normal: i32, _unused: State) };
        assert!(has_underscore_prefixed_params(&sig));
    }

    #[test]
    fn handles_no_params() {
        let sig: syn::Signature = parse_quote! { fn test() };
        assert!(!has_underscore_prefixed_params(&sig));
    }
}
