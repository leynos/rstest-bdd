//! Argument handling for step functions and fixtures.

use proc_macro2::TokenStream as TokenStream2;
use quote::quote;
use syn::{FnArg, Ident, ItemFn, Pat, Type};

#[derive(Clone)]
pub(crate) struct FixtureArg {
    pub(crate) pat: Ident,
    pub(crate) name: Ident,
    pub(crate) ty: Type,
}

#[derive(Clone)]
pub(crate) struct StepArg {
    pub(crate) pat: Ident,
    pub(crate) ty: Type,
}

pub(crate) enum Arg {
    Fixture { pat: Ident, name: Ident, ty: Type },
    Step { pat: Ident, ty: Type },
}

pub(crate) fn extract_args(func: &mut ItemFn) -> syn::Result<(Vec<FixtureArg>, Vec<StepArg>)> {
    let mut fixtures = Vec::new();
    let mut step_args = Vec::new();

    for input in &mut func.sig.inputs {
        let FnArg::Typed(arg) = input else {
            return Err(syn::Error::new_spanned(input, "methods not supported"));
        };

        let mut fixture_name = None;
        arg.attrs.retain(|a| {
            if a.path().is_ident("from") {
                fixture_name = a.parse_args::<Ident>().ok();
                false
            } else {
                true
            }
        });

        let pat = match &*arg.pat {
            Pat::Ident(i) => i.ident.clone(),
            _ => return Err(syn::Error::new_spanned(&arg.pat, "unsupported pattern")),
        };

        let ty = (*arg.ty).clone();

        if let Some(name) = fixture_name {
            fixtures.push(FixtureArg { pat, name, ty });
        } else {
            step_args.push(StepArg { pat, ty });
        }
    }

    Ok((fixtures, step_args))
}

pub(crate) fn prepare_arguments(fixtures: &[FixtureArg], step_args: &[StepArg]) -> Vec<Arg> {
    fixtures
        .iter()
        .map(|f| Arg::Fixture {
            pat: f.pat.clone(),
            name: f.name.clone(),
            ty: f.ty.clone(),
        })
        .chain(step_args.iter().map(|a| Arg::Step {
            pat: a.pat.clone(),
            ty: a.ty.clone(),
        }))
        .collect()
}

pub(crate) fn gen_arg_decls_and_idents(args: &[Arg]) -> (Vec<TokenStream2>, Vec<Ident>) {
    let mut decls = Vec::with_capacity(args.len());
    let mut idents = Vec::with_capacity(args.len());
    let mut step_idx = 0usize;

    for arg in args {
        match arg {
            Arg::Fixture { pat, name, ty } => {
                if let Type::Reference(r) = ty {
                    let inner = &*r.elem;
                    decls.push(quote! {
                        let #pat: #ty = ctx
                            .get::<#inner>(stringify!(#name))
                            .unwrap_or_else(|| panic!(
                                "missing fixture '{}' of type '{}'",
                                stringify!(#name),
                                stringify!(#inner)
                            ));
                    });
                } else {
                    decls.push(quote! {
                        let #pat: #ty = ctx
                            .get::<#ty>(stringify!(#name))
                            .unwrap_or_else(|| panic!(
                                "missing fixture '{}' of type '{}'",
                                stringify!(#name),
                                stringify!(#ty)
                            ))
                            .clone();
                    });
                }
                idents.push(pat.clone());
            }
            Arg::Step { pat, ty } => {
                let index = syn::Index::from(step_idx);
                decls.push(quote! {
                    let raw = &captures[#index];
                    let #pat: #ty = raw.parse().unwrap_or_else(|_| panic!(
                        "failed to parse placeholder '{}' with value '{}' as {}",
                        stringify!(#pat),
                        raw,
                        stringify!(#ty)
                    ));
                });
                idents.push(pat.clone());
                step_idx += 1;
            }
        }
    }

    (decls, idents)
}
