//! Utilities for handling fixtures in generated tests.

use proc_macro2::TokenStream as TokenStream2;
use quote::{format_ident, quote};

use crate::utils::result_type::try_extract_result_inner_type;

/// Generated code for wiring scenario fixture parameters into `StepContext`.
pub(crate) struct FixtureBindingCode {
    pub prelude: Vec<TokenStream2>,
    pub ctx_inserts: Vec<TokenStream2>,
    pub postlude: Vec<TokenStream2>,
    /// `true` when at least one fixture parameter has a `Result<T, E>` type,
    /// meaning the scenario must return `Result<(), E>` so the generated `?`
    /// operator can propagate initialisation errors.
    pub has_result_fixtures: bool,
}

/// Extract function argument identifiers and create insert statements.
///
/// When a fixture parameter has a `Result<T, E>` type, the generated prelude
/// unwraps it with `?` and registers the inner `T` in the `StepContext`.
/// The caller must ensure the scenario returns `Result<(), E>` so the `?`
/// operator compiles.
pub(crate) fn extract_function_fixtures(
    sig: &mut syn::Signature,
) -> syn::Result<(Vec<syn::Ident>, FixtureBindingCode)> {
    let mut counter = 0usize;
    let mut arg_idents = Vec::new();
    let mut inserts = Vec::new();
    let mut prelude = Vec::new();
    let mut postlude = Vec::new();
    let mut has_result_fixtures = false;

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
        } else if let Some(inner_ty) = try_extract_result_inner_type(ty) {
            has_result_fixtures = true;
            let unwrapped = format_ident!("__rstest_bdd_unwrapped_{cell_index}");
            prelude.push(quote! { let #unwrapped = #binding?; });
            let (pre, insert, post) =
                build_non_ref_fixture_binding(&unwrapped, &inner_ty, &name_lit, cell_index);
            prelude.push(pre);
            inserts.push(insert);
            postlude.push(post);
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
            has_result_fixtures,
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
        let ident_str = id.ident.to_string();
        return Ok(crate::utils::pattern::normalize_param_name(&ident_str).to_owned());
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
    use rstest::rstest;
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

    #[rstest]
    #[case("_world", "world")]
    #[case("__world", "_world")]
    #[case("world", "world")]
    fn resolve_fixture_name_normalizes_param(#[case] input: &str, #[case] expected: &str) {
        let ident = syn::Ident::new(input, proc_macro2::Span::call_site());
        let pat_ty: syn::PatType = parse_quote! { #ident: WorldFixture };
        #[expect(
            clippy::expect_used,
            reason = "test asserts fixture name normalization"
        )]
        let name = resolve_fixture_name(&pat_ty).expect("fixture name resolution should succeed");
        assert_eq!(name, expected);
    }

    #[test]
    #[expect(
        clippy::expect_used,
        reason = "test asserts from attribute takes precedence"
    )]
    fn resolve_fixture_name_from_attr_unchanged() {
        let sig: syn::Signature = parse_quote! { fn test(#[from(state)] _world: WorldFixture) };
        let syn::FnArg::Typed(pat_ty) = sig.inputs.first().expect("signature has one arg") else {
            panic!("expected typed argument");
        };
        let name = resolve_fixture_name(pat_ty).expect("fixture name resolution should succeed");
        assert_eq!(name, "state", "#[from] attribute should take precedence");
    }

    #[test]
    #[expect(clippy::expect_used, reason = "test asserts Result fixture extraction")]
    fn result_fixture_sets_has_result_fixtures_flag() {
        let mut sig: syn::Signature = parse_quote! {
            fn scenario(world: Result<MyWorld, String>)
        };
        let (_idents, code) =
            extract_function_fixtures(&mut sig).expect("fixture extraction should succeed");
        assert!(
            code.has_result_fixtures,
            "has_result_fixtures should be true for Result-typed fixtures"
        );
    }

    #[test]
    #[expect(
        clippy::expect_used,
        reason = "test asserts Result fixture generates unwrap statement"
    )]
    fn result_fixture_generates_unwrap_in_prelude() {
        let mut sig: syn::Signature = parse_quote! {
            fn scenario(world: Result<MyWorld, String>)
        };
        let (_idents, code) =
            extract_function_fixtures(&mut sig).expect("fixture extraction should succeed");
        let prelude_str: String = code.prelude.iter().map(ToString::to_string).collect();
        assert!(
            prelude_str.contains("__rstest_bdd_unwrapped_0"),
            "prelude should contain unwrap binding, got: {prelude_str}"
        );
        assert!(
            prelude_str.contains('?'),
            "prelude should contain ? operator for Result unwrap, got: {prelude_str}"
        );
    }

    #[test]
    #[expect(
        clippy::expect_used,
        reason = "test asserts Result fixture uses inner type for StepContext"
    )]
    fn result_fixture_uses_inner_type_for_context_insert() {
        let mut sig: syn::Signature = parse_quote! {
            fn scenario(world: Result<MyWorld, String>)
        };
        let (_idents, code) =
            extract_function_fixtures(&mut sig).expect("fixture extraction should succeed");
        let insert_str: String = code.ctx_inserts.iter().map(ToString::to_string).collect();
        assert!(
            insert_str.contains("MyWorld"),
            "context insert should use inner type MyWorld, got: {insert_str}"
        );
        assert!(
            !insert_str.contains("Result"),
            "context insert should not reference Result wrapper, got: {insert_str}"
        );
    }

    #[test]
    #[expect(
        clippy::expect_used,
        reason = "test asserts non-Result fixture does not set flag"
    )]
    fn plain_fixture_does_not_set_has_result_fixtures() {
        let mut sig: syn::Signature = parse_quote! {
            fn scenario(world: MyWorld)
        };
        let (_idents, code) =
            extract_function_fixtures(&mut sig).expect("fixture extraction should succeed");
        assert!(
            !code.has_result_fixtures,
            "has_result_fixtures should be false for plain fixtures"
        );
    }
}
