use std::collections::HashSet;

use super::{
    classify::{
        classify_datatable, classify_docstring, classify_fixture_or_step, classify_step_struct,
        extract_step_struct_attribute,
    },
    ExtractedArgs,
};

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
/// assert_eq!(args.fixtures.len(), 1);
/// assert_eq!(args.step_args.len(), 1);
/// assert!(args.datatable.is_some());
/// assert!(args.docstring.is_some());
/// assert_eq!(args.call_order.len(), 4);
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
    let mut state = ExtractedArgs {
        fixtures: vec![],
        step_args: vec![],
        step_struct: None,
        datatable: None,
        docstring: None,
        call_order: vec![],
    };

    for input in &mut func.sig.inputs {
        let syn::FnArg::Typed(arg) = input else {
            return Err(syn::Error::new_spanned(
                input,
                "methods are not supported; remove `self` from step functions",
            ));
        };
        let syn::Pat::Ident(pat_ident) = &*arg.pat else {
            return Err(syn::Error::new_spanned(
                &arg.pat,
                "unsupported parameter pattern; use a simple identifier (e.g., `arg: T`)",
            ));
        };
        let pat = pat_ident.ident.clone();
        let ty = (*arg.ty).clone();
        if extract_step_struct_attribute(arg)? {
            classify_step_struct(&mut state, arg, &pat, &ty, placeholders)?;
            continue;
        }
        let pat_str = pat.to_string();
        if placeholders.contains(&pat_str) {
            classify_fixture_or_step(&mut state, arg, pat.clone(), ty.clone(), placeholders);
            continue;
        }
        if classify_datatable(&mut state, arg, &pat, &ty)? {
            continue;
        }
        if classify_docstring(&mut state, arg, &pat, &ty)? {
            continue;
        }
        classify_fixture_or_step(&mut state, arg, pat, ty, placeholders);
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
