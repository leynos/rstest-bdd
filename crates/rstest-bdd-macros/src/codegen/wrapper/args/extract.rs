//! Parameter extraction pipeline turning step function signatures into [`Arg`] sequences.
//!
//! The module owns the orchestration of placeholder-aware classifiers, ensuring every
//! argument is processed consistently before wrapper codegen starts. Keeping this
//! summary in the header satisfies the project guideline that requires each module to
//! describe its purpose and makes the extraction flow discoverable for contributors.

use std::collections::HashSet;

use super::{
    classify::{
        classify_datatable, classify_docstring, classify_fixture_or_step, classify_step_struct,
        extract_step_struct_attribute,
    },
    ExtractedArgs,
};

type Classifier = fn(
    &mut ExtractedArgs,
    &mut syn::PatType,
    &syn::Ident,
    &syn::Type,
    &mut HashSet<String>,
) -> syn::Result<bool>;

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
            return Err(syn::Error::new_spanned(
                other,
                "unsupported parameter pattern; use a simple identifier (e.g., `arg: T`)",
            ))
        }
    };
    let ty = (*arg.ty).clone();
    Ok((arg, pat, ty))
}

fn classifier_pipeline() -> Vec<Classifier> {
    vec![
        |st, arg, pat, ty, placeholders| {
            if placeholders.contains(&pat.to_string()) {
                classify_fixture_or_step(st, arg, pat.clone(), ty.clone(), placeholders)?;
                Ok(true)
            } else {
                Ok(false)
            }
        },
        |st, arg, pat, ty, _| classify_datatable(st, arg, pat, ty),
        |st, arg, pat, ty, _| classify_docstring(st, arg, pat, ty),
        |st, arg, pat, ty, placeholders| {
            classify_fixture_or_step(st, arg, pat.clone(), ty.clone(), placeholders)?;
            Ok(true)
        },
    ]
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
    let classifiers = classifier_pipeline();

    'args: for input in &mut func.sig.inputs {
        let (arg, pat, ty) = next_typed_argument(input)?;
        if extract_step_struct_attribute(arg)? {
            classify_step_struct(&mut state, arg, placeholders)?;
            continue 'args;
        }
        for classify in &classifiers {
            if classify(&mut state, arg, &pat, &ty, placeholders)? {
                continue 'args;
            }
        }
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
