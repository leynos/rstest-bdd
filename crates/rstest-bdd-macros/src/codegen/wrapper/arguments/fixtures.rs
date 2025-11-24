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

#[derive(Copy, Clone)]
enum BorrowKind {
    Mutable,
    Immutable,
}

#[derive(Copy, Clone)]
enum ValueExtraction {
    MutRef,
    DerefValue,
    Value,
    ClonedValue,
}

fn gen_fixture_decl_inner(
    ctx: FixtureDeclContext<'_>,
    borrow_ty: &syn::Type,
    error_ty: &syn::Type,
    borrow_kind: BorrowKind,
    value_extraction: ValueExtraction,
) -> TokenStream2 {
    let missing_err = gen_missing_fixture_error(&ctx, error_ty);
    let FixtureDeclContext {
        pat,
        name,
        ty,
        ctx_ident,
        ..
    } = ctx;
    let guard_ident = format_ident!("__rstest_bdd_guard_{}", pat);

    let (guard_binding, borrow_method) = match borrow_kind {
        BorrowKind::Mutable => (quote::quote! { mut }, quote::quote! { borrow_mut }),
        BorrowKind::Immutable => (quote::quote! {}, quote::quote! { borrow_ref }),
    };

    let value_expr = match value_extraction {
        ValueExtraction::MutRef => quote::quote! { #guard_ident.value_mut() },
        ValueExtraction::DerefValue => quote::quote! { *#guard_ident.value() },
        ValueExtraction::Value => quote::quote! { #guard_ident.value() },
        ValueExtraction::ClonedValue => quote::quote! { #guard_ident.value().clone() },
    };

    quote::quote! {
        let #guard_binding #guard_ident = #ctx_ident
            .#borrow_method::<#borrow_ty>(stringify!(#name))
            .ok_or_else(|| #missing_err)?;
        let #pat: #ty = #value_expr;
    }
}

/// Generate declarations for fixture values.
///
/// Non-reference fixtures must implement [`Clone`] because wrappers clone
/// them to hand ownership to the step function.
fn gen_mut_ref_fixture_decl(ctx: FixtureDeclContext<'_>, elem: &syn::Type) -> TokenStream2 {
    gen_fixture_decl_inner(
        ctx,
        elem,
        elem,
        BorrowKind::Mutable,
        ValueExtraction::MutRef,
    )
}

fn gen_unsized_ref_fixture_decl(ctx: FixtureDeclContext<'_>, _elem: &syn::Type) -> TokenStream2 {
    gen_fixture_decl_inner(
        ctx,
        ctx.ty,
        ctx.ty,
        BorrowKind::Immutable,
        ValueExtraction::DerefValue,
    )
}

fn gen_sized_ref_fixture_decl(ctx: FixtureDeclContext<'_>, elem: &syn::Type) -> TokenStream2 {
    gen_fixture_decl_inner(
        ctx,
        elem,
        elem,
        BorrowKind::Immutable,
        ValueExtraction::Value,
    )
}

fn gen_owned_fixture_decl(ctx: FixtureDeclContext<'_>) -> TokenStream2 {
    gen_fixture_decl_inner(
        ctx,
        ctx.ty,
        ctx.ty,
        BorrowKind::Immutable,
        ValueExtraction::ClonedValue,
    )
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
