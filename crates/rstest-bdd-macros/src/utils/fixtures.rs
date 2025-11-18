//! Utilities for handling fixtures in generated tests.

use proc_macro2::TokenStream as TokenStream2;
use quote::quote;

pub(crate) struct FixtureBindingCode {
    pub prelude: Vec<TokenStream2>,
    pub ctx_inserts: Vec<TokenStream2>,
    pub postlude: Vec<TokenStream2>,
}

/// Extract function argument identifiers and create insert statements.
pub(crate) fn extract_function_fixtures(
    sig: &mut syn::Signature,
) -> syn::Result<(Vec<syn::Ident>, FixtureBindingCode)> {
    let mut counter = 0usize;
    let mut arg_idents = Vec::new();
    let mut inserts = Vec::new();
    let mut prelude = Vec::new();
    let mut postlude = Vec::new();

    for input in &mut sig.inputs {
        let syn::FnArg::Typed(pat_ty) = input else {
            continue;
        };

        let fixture_name = resolve_fixture_name(pat_ty)?;
        let binding = ensure_binding_ident(pat_ty, counter)?;
        counter += 1;

        let name_lit = syn::LitStr::new(&fixture_name, proc_macro2::Span::call_site());
        arg_idents.push(binding.clone());
        let ty = &*pat_ty.ty;
        if matches!(ty, syn::Type::Reference(_)) {
            inserts.push(quote! { ctx.insert(#name_lit, &#binding); });
        } else {
            let cell_ident = syn::Ident::new(
                &format!("__rstest_bdd_cell_{binding}"),
                proc_macro2::Span::call_site(),
            );
            prelude.push(quote! {
                #[allow(unused_mut)]
                let mut #binding = #binding;
                let #cell_ident = ::std::cell::RefCell::new(Box::new(#binding));
            });
            inserts.push(quote! { ctx.insert_owned::<#ty>(#name_lit, &#cell_ident); });
            postlude.push(quote! {
                #[allow(unused_mut)]
                let mut #binding = *#cell_ident.into_inner();
            });
        }
    }

    Ok((
        arg_idents,
        FixtureBindingCode {
            prelude,
            ctx_inserts: inserts,
            postlude,
        },
    ))
}

fn ensure_binding_ident(pat_ty: &mut syn::PatType, counter: usize) -> syn::Result<syn::Ident> {
    match &mut *pat_ty.pat {
        syn::Pat::Ident(id) => Ok(id.ident.clone()),
        syn::Pat::Wild(_) => {
            let ident = syn::Ident::new(
                &format!("__rstest_bdd_fixture_{counter}"),
                proc_macro2::Span::call_site(),
            );
            pat_ty.pat = Box::new(syn::Pat::Ident(syn::PatIdent {
                attrs: Vec::new(),
                by_ref: None,
                mutability: None,
                ident: ident.clone(),
                subpat: None,
            }));
            Ok(ident)
        }
        pat => Err(syn::Error::new_spanned(
            pat,
            "scenario fixtures must bind to an identifier; use `_` with #[from(...)] to ignore it",
        )),
    }
}

fn resolve_fixture_name(pat_ty: &syn::PatType) -> syn::Result<String> {
    if let Some(path) = find_from_attr(&pat_ty.attrs)? {
        let Some(last) = path.segments.last() else {
            return Err(syn::Error::new_spanned(path, "expected fixture path"));
        };
        return Ok(last.ident.to_string());
    }
    if let syn::Pat::Ident(id) = &*pat_ty.pat {
        return Ok(id.ident.to_string());
    }
    Err(syn::Error::new_spanned(
        &pat_ty.pat,
        "fixture patterns without an identifier must specify the source with #[from(...)]",
    ))
}

fn find_from_attr(attrs: &[syn::Attribute]) -> syn::Result<Option<syn::Path>> {
    for attr in attrs {
        if attr
            .path()
            .segments
            .last()
            .is_some_and(|segment| segment.ident == "from")
        {
            return Ok(Some(attr.parse_args::<syn::Path>()?));
        }
    }
    Ok(None)
}
