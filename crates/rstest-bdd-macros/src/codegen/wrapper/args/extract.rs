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
        extract_step_struct_attribute, ClassificationContext,
    },
    ExtractedArgs,
};

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

        let mut needs_fixture_or_step = {
            let pat_name = pat.to_string();
            placeholders.contains(&pat_name)
        };

        if !needs_fixture_or_step {
            if classify_datatable(&mut state, arg, &pat, &ty)? {
                continue 'args;
            }

            if classify_docstring(&mut state, arg, &pat, &ty)? {
                continue 'args;
            }

            needs_fixture_or_step = true;
        }

        if needs_fixture_or_step {
            let mut ctx = ClassificationContext::new(&mut state, placeholders);
            classify_fixture_or_step(&mut ctx, arg, pat, ty)?;
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
