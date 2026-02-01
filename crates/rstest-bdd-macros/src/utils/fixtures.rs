//! Utilities for handling fixtures in generated tests.

use proc_macro2::TokenStream as TokenStream2;
use quote::{format_ident, quote};

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
        let cell_index = counter;
        counter += 1;

        let name_lit = syn::LitStr::new(&fixture_name, proc_macro2::Span::call_site());
        arg_idents.push(binding.clone());
        let ty = &*pat_ty.ty;
        if matches!(ty, syn::Type::Reference(_)) {
            inserts.push(quote! { ctx.insert(#name_lit, &#binding); });
        } else {
            let (pre, insert, post) =
                build_non_ref_fixture_binding(&binding, ty, &name_lit, cell_index);
            prelude.push(pre);
            inserts.push(insert);
            postlude.push(post);
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

fn build_non_ref_fixture_binding(
    binding: &syn::Ident,
    ty: &syn::Type,
    name_lit: &syn::LitStr,
    cell_index: usize,
) -> (TokenStream2, TokenStream2, TokenStream2) {
    let cell_ident = format_ident!("__rstest_bdd_cell_{cell_index}");

    let prelude = quote! {
        let #cell_ident: ::std::cell::RefCell<Box<dyn ::std::any::Any>> =
            ::std::cell::RefCell::new(Box::new(#binding));
    };
    let insert = quote! {
        ctx.insert_owned::<#ty>(#name_lit, &#cell_ident);
    };
    let postlude = quote! {
        #[expect(
            unused_mut,
            reason = "binding is declared mutable to allow user code in step implementations to mutate it, but the generated code may not perform any mutation",
        )]
        let mut #binding = *#cell_ident
            .into_inner()
            .downcast::<#ty>()
            .expect("generated fixture type must match binding");
    };

    (prelude, insert, postlude)
}

fn ensure_binding_ident(pat_ty: &mut syn::PatType, counter: usize) -> syn::Result<syn::Ident> {
    match &mut *pat_ty.pat {
        syn::Pat::Ident(id) => Ok(id.ident.clone()),
        syn::Pat::Wild(_) => {
            let ident = syn::Ident::new(
                &format!("__rstest_bdd_fixture_{counter}"),
                proc_macro2::Span::call_site(),
            );
            *pat_ty.pat = syn::Pat::Ident(syn::PatIdent {
                attrs: Vec::new(),
                by_ref: None,
                mutability: None,
                ident: ident.clone(),
                subpat: None,
            });
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

#[cfg(test)]
mod tests {
    use super::*;
    use syn::parse_quote;

    #[test]
    #[expect(
        clippy::expect_used,
        reason = "test asserts fixture extraction for underscore bindings"
    )]
    fn non_ref_fixture_cell_ident_uses_index() {
        let mut sig: syn::Signature = parse_quote! {
            fn scenario(_state: MyState)
        };
        let (_idents, code) =
            extract_function_fixtures(&mut sig).expect("fixture extraction should succeed");
        let prelude = code
            .prelude
            .first()
            .expect("owned fixtures should generate a prelude binding");
        let prelude_str = prelude.to_string();
        assert!(
            prelude_str.contains("__rstest_bdd_cell_0"),
            "cell identifier should use the fixture index for underscore bindings"
        );
        assert!(
            !prelude_str.contains("__rstest_bdd_cell__"),
            "cell identifier should not embed the raw underscore binding"
        );
    }
}
