//! Parameter extraction pipeline turning step function signatures into [`Arg`] sequences.
//!
//! The module owns the orchestration of placeholder-aware classifiers, ensuring every
//! argument is processed consistently before wrapper codegen starts. Keeping this
//! summary in the header satisfies the project guideline that requires each module to
//! describe its purpose and makes the extraction flow discoverable for contributors.

use std::collections::HashSet;

use proc_macro2::TokenTree;
use quote::ToTokens;

use super::{
    ExtractedArgs,
    classify::{
        ClassificationContext, classify_datatable, classify_docstring, classify_fixture_or_step,
        classify_step_struct, extract_step_struct_attribute,
    },
};

fn pattern_display_for_error(pat: &syn::Pat) -> String {
    pat.to_token_stream().to_string()
}

fn span_for_pattern(pat: &syn::Pat) -> proc_macro2::Span {
    let tokens = pat.to_token_stream();
    let mut iter = tokens.into_iter();
    let Some(first) = iter.next() else {
        return proc_macro2::Span::call_site();
    };
    let mut last = first.clone();
    for token in iter {
        last = token;
    }
    let first_span = span_for_token_tree(&first);
    let last_span = span_for_token_tree(&last);
    first_span.join(last_span).unwrap_or(first_span)
}

fn span_for_token_tree(token: &TokenTree) -> proc_macro2::Span {
    match token {
        TokenTree::Group(group) => group.span(),
        TokenTree::Ident(ident) => ident.span(),
        TokenTree::Punct(punct) => punct.span(),
        TokenTree::Literal(literal) => literal.span(),
    }
}

fn next_typed_argument(
    input: &mut syn::FnArg,
) -> syn::Result<(&mut syn::PatType, syn::Ident, syn::Type)> {
    let syn::FnArg::Typed(arg) = input else {
        return Err(syn::Error::new_spanned(
            input,
            "methods are not supported; remove `self` from step functions",
        ));
    };
    let pat = match &*arg.pat {
        syn::Pat::Ident(pat_ident) => pat_ident.ident.clone(),
        other => {
            let pattern = pattern_display_for_error(other);
            return Err(syn::Error::new(
                span_for_pattern(other),
                format!(
                    "unsupported parameter pattern `{pattern}`; use a simple identifier (e.g., `arg: T`)"
                ),
            ));
        }
    };
    let ty = (*arg.ty).clone();
    Ok((arg, pat, ty))
}

/// Classifies an argument as a special argument (datatable or docstring) before
/// the pipeline falls back to fixtures/placeholders. Returns `Ok(true)` if one
/// of the special classifiers consumed the argument.
fn classify_special_argument(
    state: &mut ExtractedArgs,
    arg: &mut syn::PatType,
    pat: &syn::Ident,
    ty: &syn::Type,
) -> syn::Result<bool> {
    if classify_datatable(state, arg, pat, ty)? {
        return Ok(true);
    }
    if classify_docstring(state, arg, pat, ty)? {
        return Ok(true);
    }
    Ok(false)
}

/// Classifies an argument as either a fixture or step argument after all
/// special cases have been evaluated.
fn classify_step_or_fixture(
    ctx: &mut ClassificationContext,
    arg: &mut syn::PatType,
) -> syn::Result<()> {
    let pat = match &*arg.pat {
        syn::Pat::Ident(pat_ident) => pat_ident.ident.clone(),
        other => {
            let pattern = pattern_display_for_error(other);
            return Err(syn::Error::new(
                span_for_pattern(other),
                format!(
                    "unsupported parameter pattern `{pattern}`; use a simple identifier (e.g., `arg: T`)"
                ),
            ));
        }
    };
    let ty = (*arg.ty).clone();
    classify_fixture_or_step(ctx, arg, pat, ty).map(|_| ())
}

/// Extract fixture, step, data table, and doc string arguments from a function signature.
///
/// # Examples
/// ```rust,ignore
/// use syn::parse_quote;
///
/// let mut func: syn::ItemFn = parse_quote! {
///     fn step(#[from] a: usize, datatable: Vec<Vec<String>>, docstring: String, b: i32) {}
/// };
/// let mut placeholders = std::collections::HashSet::new();
/// placeholders.insert("b".into());
/// let args = extract_args(&mut func, &mut placeholders).unwrap();
/// assert_eq!(args.args.len(), 4);
/// let has_datatable = args
///     .args
///     .iter()
///     .any(|arg| matches!(arg, super::Arg::DataTable { .. }));
/// assert!(has_datatable);
/// let has_docstring = args
///     .args
///     .iter()
///     .any(|arg| matches!(arg, super::Arg::DocString { .. }));
/// assert!(has_docstring);
/// ```
///
/// Note: special arguments must use the canonical names:
/// - data table parameter must be annotated with `#[datatable]` or be named
///   `datatable` and have type `Vec<Vec<String>>`
/// - doc string parameter must be named `docstring` and have type `String`
///
/// At most one `datatable` and one `docstring` parameter are permitted.
// FIXME: https://github.com/leynos/rstest-bdd/issues/54
pub fn extract_args(
    func: &mut syn::ItemFn,
    placeholders: &mut HashSet<String>,
) -> syn::Result<ExtractedArgs> {
    let mut state = ExtractedArgs::default();

    'args: for input in &mut func.sig.inputs {
        let (arg, pat, ty) = next_typed_argument(input)?;
        if extract_step_struct_attribute(arg)? {
            classify_step_struct(&mut state, arg, placeholders)?;
            continue 'args;
        }

        let is_placeholder = placeholders.contains(&pat.to_string());
        if !is_placeholder && classify_special_argument(&mut state, arg, &pat, &ty)? {
            continue 'args;
        }

        let mut ctx = ClassificationContext::new(&mut state, placeholders);
        classify_step_or_fixture(&mut ctx, arg)?;
    }
    if !placeholders.is_empty() {
        let mut missing: Vec<_> = placeholders.iter().cloned().collect();
        missing.sort();
        let missing = missing.join(", ");
        return Err(syn::Error::new(
            func.sig.ident.span(),
            format!("missing step arguments for placeholders: {missing}"),
        ));
    }
    Ok(state)
}

#[cfg(test)]
mod tests {
    use super::*;
    use syn::parse_quote;

    #[test]
    fn classify_step_or_fixture_reports_pattern_in_error() {
        let mut extracted = ExtractedArgs::default();
        let mut placeholders = HashSet::new();
        let mut ctx = ClassificationContext::new(&mut extracted, &mut placeholders);
        let mut arg: syn::PatType = parse_quote!((value, other): usize);

        let Err(err) = classify_step_or_fixture(&mut ctx, &mut arg) else {
            panic!("non-identifier patterns must error");
        };
        let msg = err.to_string();
        assert!(msg.contains("unsupported parameter pattern"));
        assert!(msg.contains("value"));
        assert!(msg.contains("other"));
    }
}
