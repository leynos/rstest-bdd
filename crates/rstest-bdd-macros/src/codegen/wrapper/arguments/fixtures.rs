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
    ClonedValue,
}

#[derive(Copy, Clone)]
struct FixtureDeclConfig<'a> {
    borrow_ty: &'a syn::Type,
    error_ty: &'a syn::Type,
    borrow_kind: BorrowKind,
    value_extraction: ValueExtraction,
}

impl<'a> FixtureDeclConfig<'a> {
    const fn new(
        borrow_ty: &'a syn::Type,
        error_ty: &'a syn::Type,
        borrow_kind: BorrowKind,
        value_extraction: ValueExtraction,
    ) -> Self {
        Self {
            borrow_ty,
            error_ty,
            borrow_kind,
            value_extraction,
        }
    }
}

fn gen_fixture_decl_inner(
    ctx: FixtureDeclContext<'_>,
    config: FixtureDeclConfig<'_>,
) -> TokenStream2 {
    let missing_err = gen_missing_fixture_error(&ctx, config.error_ty);
    let FixtureDeclContext {
        pat,
        name,
        ty,
        ctx_ident,
        ..
    } = ctx;
    let guard_ident = format_ident!("__rstest_bdd_guard_{}", pat);

    let (guard_binding, borrow_method) = match config.borrow_kind {
        BorrowKind::Mutable => (quote::quote! { mut }, quote::quote! { borrow_mut }),
        BorrowKind::Immutable => (quote::quote! {}, quote::quote! { borrow_ref }),
    };

    let borrow_ty = config.borrow_ty;

    let value_expr = match config.value_extraction {
        ValueExtraction::MutRef => quote::quote! { #guard_ident.value_mut() },
        ValueExtraction::DerefValue => quote::quote! { *#guard_ident.value() },
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
    let config = FixtureDeclConfig::new(elem, elem, BorrowKind::Mutable, ValueExtraction::MutRef);
    gen_fixture_decl_inner(ctx, config)
}

fn gen_unsized_ref_fixture_decl(ctx: FixtureDeclContext<'_>, _elem: &syn::Type) -> TokenStream2 {
    let config = FixtureDeclConfig::new(
        ctx.ty,
        ctx.ty,
        BorrowKind::Immutable,
        ValueExtraction::DerefValue,
    );
    gen_fixture_decl_inner(ctx, config)
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
    let guard_ident = format_ident!("__rstest_bdd_guard_{}", pat);
    let guard_enum_ident = format_ident!("__rstest_bdd_guard_enum_{}", pat);
    let elem_ref_ty = quote::quote! { &'static #elem };

    quote::quote! {
        #[allow(non_camel_case_types)]
        enum #guard_enum_ident<'a> {
            Owned(#path::FixtureRef<'a, #elem>),
            Shared(#path::FixtureRef<'a, #elem_ref_ty>),
        }

        let #guard_ident = if let Some(guard) = #ctx_ident.borrow_ref::<#elem>(stringify!(#name)) {
            #guard_enum_ident::Owned(guard)
        } else {
            #guard_enum_ident::Shared(
                #ctx_ident
                    .borrow_ref::<#elem_ref_ty>(stringify!(#name))
                    .ok_or_else(|| #missing_err)?
            )
        };

        let #pat: #ty = match &#guard_ident {
            #guard_enum_ident::Owned(g) => g.value(),
            #guard_enum_ident::Shared(g) => *g.value(),
        };
    }
}

fn gen_owned_fixture_decl(ctx: FixtureDeclContext<'_>) -> TokenStream2 {
    let config = FixtureDeclConfig::new(
        ctx.ty,
        ctx.ty,
        BorrowKind::Immutable,
        ValueExtraction::ClonedValue,
    );
    gen_fixture_decl_inner(ctx, config)
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
