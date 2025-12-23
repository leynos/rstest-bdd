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

/// Compute a best-effort span for a parameter pattern.
///
/// We prefer producing a span that covers the whole pattern so rustc can
/// underline the full destructuring region. On stable toolchains, procedural
/// macro spans cannot always be joined across tokens, so we fall back to a
/// reasonably informative token span (e.g. the group containing the tuple or
/// struct fields).
fn span_for_pattern(pat: &syn::Pat) -> proc_macro2::Span {
    let tokens: Vec<_> = pat.to_token_stream().into_iter().collect();
    debug_assert!(
        !tokens.is_empty(),
        "syn::Pat should not produce an empty token stream"
    );
    let Some(first) = tokens.first() else {
        return proc_macro2::Span::call_site();
    };
    let Some(last) = tokens.last() else {
        return proc_macro2::Span::call_site();
    };

    let first_span = span_for_token_tree(first);
    let last_span = span_for_token_tree(last);
    first_span.join(last_span).unwrap_or_else(|| {
        tokens
            .iter()
            .rev()
            .find_map(|token| match token {
                TokenTree::Group(group) => Some(group.span()),
                _ => None,
            })
            .unwrap_or(first_span)
    })
}

/// Extract the span from a single token tree node.
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
            let pattern = other.to_token_stream().to_string();
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
            let pattern = other.to_token_stream().to_string();
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

    fn parse_fn(src: &str) -> syn::ItemFn {
        match syn::parse_str(src) {
            Ok(func) => func,
            Err(err) => panic!("test input should parse: {err}"),
        }
    }

    fn first_input(func: syn::ItemFn) -> syn::FnArg {
        let Some(arg) = func.sig.inputs.into_iter().next() else {
            panic!("test input should contain one argument");
        };
        arg
    }

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

    #[test]
    fn next_typed_argument_reports_pattern_in_error() {
        let src = "fn step((a, b): (i32, i32)) {}";
        let func = parse_fn(src);
        let mut input = first_input(func);

        let Err(err) = next_typed_argument(&mut input) else {
            panic!("tuple patterns must error");
        };

        let msg = err.to_string();
        assert!(
            msg.contains("unsupported parameter pattern"),
            "unexpected: {msg}"
        );
        assert!(msg.contains('a'), "unexpected: {msg}");
        assert!(msg.contains('b'), "unexpected: {msg}");

        let Some(expected_start) = src.find("(a, b)") else {
            panic!("test input should contain tuple pattern");
        };
        assert_eq!(err.span().start().line, 1);
        assert_eq!(err.span().start().column, expected_start);
    }

    #[test]
    fn span_for_pattern_handles_single_token() {
        let src = "fn step(value: i32) {}";
        let func = parse_fn(src);
        let arg = first_input(func);
        let syn::FnArg::Typed(pat_ty) = arg else {
            panic!("test input should contain a typed argument");
        };

        let span = span_for_pattern(&pat_ty.pat);
        let Some(expected_start) = src.find("value") else {
            panic!("test input should contain identifier");
        };
        assert_eq!(span.start().line, 1);
        assert_eq!(span.end().line, 1);
        assert_eq!(span.start().column, expected_start);
        assert!(
            span.end().column > span.start().column,
            "unexpected span end: {:?}",
            span.end()
        );
    }

    #[test]
    fn span_for_pattern_handles_multi_token() {
        let src = "fn step((value, other): (i32, i32)) {}";
        let func = parse_fn(src);
        let arg = first_input(func);
        let syn::FnArg::Typed(pat_ty) = arg else {
            panic!("test input should contain a typed argument");
        };

        let span = span_for_pattern(&pat_ty.pat);
        let Some(expected_start) = src.find("(value, other)") else {
            panic!("test input should contain tuple pattern");
        };
        assert_eq!(span.start().line, 1);
        assert_eq!(span.end().line, 1);
        assert_eq!(span.start().column, expected_start);
        assert!(
            span.end().column > span.start().column,
            "unexpected span end: {:?}",
            span.end()
        );
    }

    #[test]
    fn span_for_pattern_points_to_full_destructuring_pattern() {
        let src = "fn step(User { name }: User) {}";
        let func = parse_fn(src);
        let mut input = first_input(func);

        let Err(err) = next_typed_argument(&mut input) else {
            panic!("struct destructuring patterns must error");
        };

        let Some(pattern_start) = src.find("User { name }") else {
            panic!("test input should contain struct pattern");
        };
        assert_eq!(err.span().start().line, 1);
        assert_eq!(err.span().start().column, pattern_start);
        assert!(
            err.span().end().column > err.span().start().column,
            "expected span to cover at least one token"
        );
    }
}
