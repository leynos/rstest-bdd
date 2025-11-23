//! Fixture declaration code emitted into generated step wrappers.
use proc_macro2::TokenStream as TokenStream2;
use quote::format_ident;

use super::super::args::Arg;
use crate::codegen::rstest_bdd_path;

/// Context for generating fixture declarations in step wrappers.
#[derive(Copy, Clone)]
struct FixtureDeclContext<'a> {
    pat: &'a syn::Ident,
    name: &'a syn::Ident,
    ty: &'a syn::Type,
    ident: &'a syn::Ident,
    ctx_ident: &'a proc_macro2::Ident,
}

/// Generate error for missing fixture.
fn gen_missing_fixture_error(ctx: &FixtureDeclContext<'_>, fixture_ty: &syn::Type) -> TokenStream2 {
    let path = rstest_bdd_path();
    let FixtureDeclContext { name, ident, .. } = ctx;
    quote::quote! {
        #path::StepError::MissingFixture {
            name: stringify!(#name).to_string(),
            ty: stringify!(#fixture_ty).to_string(),
            step: stringify!(#ident).to_string(),
        }
    }
}

/// Generate declarations for fixture values.
///
/// Non-reference fixtures must implement [`Clone`] because wrappers clone
/// them to hand ownership to the step function.
fn gen_mut_ref_fixture_decl(ctx: FixtureDeclContext<'_>, elem: &syn::Type) -> TokenStream2 {
    let missing_err = gen_missing_fixture_error(&ctx, elem);
    let FixtureDeclContext {
        pat,
        name,
        ty,
        ctx_ident,
        ..
    } = ctx;
    let guard_ident = format_ident!("__rstest_bdd_guard_{}", pat);
    quote::quote! {
        let mut #guard_ident = #ctx_ident
            .borrow_mut::<#elem>(stringify!(#name))
            .ok_or_else(|| #missing_err)?;
        let #pat: #ty = #guard_ident.value_mut();
    }
}

fn gen_unsized_ref_fixture_decl(ctx: FixtureDeclContext<'_>, elem: &syn::Type) -> TokenStream2 {
    let missing_err = gen_missing_fixture_error(&ctx, elem);
    let FixtureDeclContext {
        pat,
        name,
        ty,
        ctx_ident,
        ..
    } = ctx;
    let guard_ident = format_ident!("__rstest_bdd_guard_{}", pat);
    quote::quote! {
        let #guard_ident = #ctx_ident
            .borrow_ref::<#ty>(stringify!(#name))
            .ok_or_else(|| #missing_err)?;
        let #pat: #ty = #guard_ident.value();
    }
}

fn gen_sized_ref_fixture_decl(ctx: FixtureDeclContext<'_>, elem: &syn::Type) -> TokenStream2 {
    let missing_err = gen_missing_fixture_error(&ctx, elem);
    let FixtureDeclContext {
        pat,
        name,
        ty,
        ctx_ident,
        ..
    } = ctx;
    let path = rstest_bdd_path();
    let owned_guard = format_ident!("__rstest_bdd_guard_owned_{}", pat);
    let shared_guard = format_ident!("__rstest_bdd_guard_shared_{}", pat);
    quote::quote! {
        let mut #owned_guard: ::std::option::Option<#path::FixtureRef<'_, #elem>> = None;
        let mut #shared_guard: ::std::option::Option<#path::FixtureRef<'_, #ty>> = None;
        let #pat: #ty;
        if let Some(guard) = #ctx_ident.borrow_ref::<#elem>(stringify!(#name)) {
            #owned_guard = Some(guard);
            #pat = #owned_guard
                .as_ref()
                .expect("fixture guard stored")
                .value();
        } else {
            #shared_guard = Some(
                #ctx_ident
                    .borrow_ref::<#ty>(stringify!(#name))
                    .ok_or_else(|| #missing_err)?
            );
            #pat = *#shared_guard
                .as_ref()
                .expect("fixture guard stored")
                .value();
        }
    }
}

fn gen_owned_fixture_decl(ctx: FixtureDeclContext<'_>) -> TokenStream2 {
    let missing_err = gen_missing_fixture_error(&ctx, ctx.ty);
    let FixtureDeclContext {
        pat,
        name,
        ty,
        ctx_ident,
        ..
    } = ctx;
    let guard_ident = format_ident!("__rstest_bdd_guard_{}", pat);
    quote::quote! {
        let #guard_ident = #ctx_ident
            .borrow_ref::<#ty>(stringify!(#name))
            .ok_or_else(|| #missing_err)?;
        let #pat: #ty = #guard_ident.value().clone();
    }
}

pub(super) fn gen_fixture_decls(
    fixtures: &[&Arg],
    ident: &syn::Ident,
    ctx_ident: &proc_macro2::Ident,
) -> Vec<TokenStream2> {
    fixtures
        .iter()
        .map(|fixture| {
            let Arg::Fixture { pat, name, ty } = fixture else {
                unreachable!("fixture vector must contain fixtures");
            };
            let ctx = FixtureDeclContext {
                pat,
                name,
                ty,
                ident,
                ctx_ident,
            };
            match ty {
                syn::Type::Reference(reference) if reference.mutability.is_some() => {
                    let elem = &*reference.elem;
                    gen_mut_ref_fixture_decl(ctx, elem)
                }
                syn::Type::Reference(reference) => {
                    let elem = &*reference.elem;
                    if is_unsized_reference_target(elem) {
                        gen_unsized_ref_fixture_decl(ctx, elem)
                    } else {
                        gen_sized_ref_fixture_decl(ctx, elem)
                    }
                }
                _ => gen_owned_fixture_decl(ctx),
            }
        })
        .collect()
}

fn is_unsized_reference_target(ty: &syn::Type) -> bool {
    matches!(
        ty,
        syn::Type::Slice(_) | syn::Type::TraitObject(_) | syn::Type::ImplTrait(_)
    ) || matches!(
        ty,
        syn::Type::Path(path) if path.qself.is_none() && path.path.is_ident("str")
    )
}
