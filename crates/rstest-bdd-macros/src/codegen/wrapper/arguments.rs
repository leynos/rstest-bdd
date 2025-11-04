//! Argument code generation utilities shared by wrapper emission logic.

use super::args::{
    ArgumentCollections, CallArg, DataTableArg, DocStringArg, FixtureArg, StepArg, StepArgStruct,
};
use proc_macro2::TokenStream as TokenStream2;
use quote::{format_ident, quote};

#[derive(Copy, Clone)]
pub(super) struct StepMeta<'a> {
    pub(super) pattern: &'a syn::LitStr,
    pub(super) ident: &'a syn::Ident,
}

pub(super) struct PreparedArgs {
    pub(super) declares: Vec<TokenStream2>,
    pub(super) step_arg_parses: Vec<TokenStream2>,
    pub(super) step_struct_decl: Option<TokenStream2>,
    pub(super) datatable_decl: Option<TokenStream2>,
    pub(super) docstring_decl: Option<TokenStream2>,
}

/// Quote construction for [`StepError`] variants sharing `pattern`,
/// `function` and `message` fields.
pub(super) fn step_error_tokens(
    variant: &syn::Ident,
    pattern: &syn::LitStr,
    ident: &syn::Ident,
    message: &TokenStream2,
) -> TokenStream2 {
    let path = crate::codegen::rstest_bdd_path();
    quote! {
        #path::StepError::#variant {
            pattern: #pattern.to_string(),
            function: stringify!(#ident).to_string(),
            message: #message,
        }
    }
}

fn gen_optional_decl<T, F>(
    arg: Option<&T>,
    meta: StepMeta<'_>,
    error_msg: &str,
    generator: F,
) -> Option<TokenStream2>
where
    F: FnOnce(&T) -> (syn::Ident, TokenStream2, TokenStream2),
{
    arg.map(|arg_value| {
        let (pat, ty, expr) = generator(arg_value);
        let StepMeta { pattern, ident } = meta;
        let missing_err = step_error_tokens(
            &format_ident!("ExecutionError"),
            pattern,
            ident,
            &quote! { format!("Step '{}' {}", #pattern, #error_msg) },
        );
        let convert_err = step_error_tokens(
            &format_ident!("ExecutionError"),
            pattern,
            ident,
            &quote! { format!("failed to convert auxiliary argument for step '{}': {}", #pattern, e) },
        );
        quote! {
            let #pat: #ty = #expr
                .ok_or_else(|| #missing_err)?
                .try_into()
                .map_err(|e| #convert_err)?;
        }
    })
}

/// Generate declaration for a data table argument.
pub(super) fn gen_datatable_decl(
    datatable: Option<&DataTableArg>,
    pattern: &syn::LitStr,
    ident: &syn::Ident,
) -> Option<TokenStream2> {
    gen_optional_decl(
        datatable,
        StepMeta { pattern, ident },
        "requires a data table",
        |DataTableArg { pat, ty }| {
            let pat = pat.clone();
            let declared_ty = ty.clone();
            let ty = quote! { #declared_ty };
            let expr = quote! {
                _table.map(|t| {
                    t.iter()
                        .map(|row| row.iter().map(|cell| cell.to_string()).collect::<Vec<String>>())
                        .collect::<Vec<Vec<String>>>()
                })
            };
            (pat, ty, expr)
        },
    )
}

/// Generate declaration for a doc string argument.
///
/// Step functions require an owned `String`, so the wrapper copies the block.
pub(super) fn gen_docstring_decl(
    docstring: Option<&DocStringArg>,
    pattern: &syn::LitStr,
    ident: &syn::Ident,
) -> Option<TokenStream2> {
    gen_optional_decl(
        docstring,
        StepMeta { pattern, ident },
        "requires a doc string",
        |DocStringArg { pat }| {
            let pat = pat.clone();
            let ty = quote! { String };
            let expr = quote! { _docstring.map(|s| s.to_owned()) };
            (pat, ty, expr)
        },
    )
}

fn gen_step_struct_decl(
    step_struct: Option<&StepArgStruct>,
    captures: &[TokenStream2],
    placeholder_names: &[syn::LitStr],
    meta: StepMeta<'_>,
) -> Option<TokenStream2> {
    let capture_count = placeholder_names.len();
    step_struct.map(|StepArgStruct { pat, ty }| {
        let StepMeta { pattern, ident } = meta;
        let values_ident = format_ident!("__rstest_bdd_struct_values");
        let missing_errs: Vec<_> = placeholder_names
            .iter()
            .map(|name| {
                step_error_tokens(
                    &format_ident!("ExecutionError"),
                    pattern,
                    ident,
                    &quote! {
                        format!(
                            "pattern '{}' missing capture for placeholder '{{{}}}' required by '{}'",
                            #pattern,
                            #name,
                            stringify!(#pat),
                        )
                    },
                )
            })
            .collect();
        let convert_err = step_error_tokens(
            &format_ident!("ExecutionError"),
            pattern,
            ident,
            &quote! {
                format!(
                    "failed to populate '{}' from pattern '{}': {}",
                    stringify!(#pat),
                    #pattern,
                    error
                )
            },
        );
        let capture_inits = captures.iter().zip(missing_errs.iter()).map(|(capture, missing)| {
            quote! {
                let raw = #capture.ok_or_else(|| #missing)?;
                #values_ident.push(raw.to_string());
            }
        });
        quote! {
            let mut #values_ident = Vec::with_capacity(#capture_count);
            #(#capture_inits)*
            let #pat: #ty = ::std::convert::TryFrom::try_from(#values_ident)
                .map_err(|error| #convert_err)?;
        }
    })
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

/// Generate declarations for fixture values.
///
/// Non-reference fixtures must implement [`Clone`] because wrappers clone
/// them to hand ownership to the step function.
pub(super) fn gen_fixture_decls(
    fixtures: &[FixtureArg],
    ident: &syn::Ident,
    ctx_ident: &proc_macro2::Ident,
) -> Vec<TokenStream2> {
    fixtures
        .iter()
        .map(|FixtureArg { pat, name, ty }| {
            let path = crate::codegen::rstest_bdd_path();
            let (lookup_ty, post_get, ty_label) = match ty {
                syn::Type::Reference(reference) if reference.mutability.is_some() => (
                    quote! { #ty },
                    quote! { .map(|value| &mut **value) },
                    quote! { stringify!(#ty) },
                ),
                syn::Type::Reference(reference) => {
                    let elem = &*reference.elem;
                    if is_unsized_reference_target(elem) {
                        (
                            quote! { #ty },
                            quote! { .copied() },
                            quote! { stringify!(#ty) },
                        )
                    } else {
                        (
                            quote! { #elem },
                            TokenStream2::new(),
                            quote! { stringify!(#elem) },
                        )
                    }
                }
                _ => (
                    quote! { #ty },
                    quote! { .cloned() },
                    quote! { stringify!(#ty) },
                ),
            };
            quote! {
                let #pat: #ty = #ctx_ident
                    .get::<#lookup_ty>(stringify!(#name))
                    #post_get
                    .ok_or_else(|| #path::StepError::MissingFixture {
                        name: stringify!(#name).to_string(),
                        ty: (#ty_label).to_string(),
                        step: stringify!(#ident).to_string(),
                    })?;
            }
        })
        .collect()
}

/// Generate code to parse step arguments from regex captures.
pub(super) fn gen_step_parses(
    step_args: &[StepArg],
    captured: &[TokenStream2],
    meta: StepMeta<'_>,
) -> Vec<TokenStream2> {
    let StepMeta { pattern, ident } = meta;
    step_args
        .iter()
        .zip(captured.iter().enumerate())
        .map(|(StepArg { pat, ty }, (idx, capture))| {
            let raw_ident = format_ident!("__raw{}", idx);
            let missing_cap_err = step_error_tokens(
                &format_ident!("ExecutionError"),
                pattern,
                ident,
                &quote! {
                    format!(
                        "pattern '{}' missing capture for argument '{}'",
                        #pattern,
                        stringify!(#pat),
                    )
                },
            );
            let parse_err = step_error_tokens(
                &format_ident!("ExecutionError"),
                pattern,
                ident,
                &quote! {
                    format!(
                        "failed to parse argument '{}' of type '{}' from pattern '{}' with captured value: '{:?}'",
                        stringify!(#pat),
                        stringify!(#ty),
                        #pattern,
                        #raw_ident,
                    )
                },
            );
            quote! {
                let #raw_ident = #capture.ok_or_else(|| #missing_cap_err)?;
                let #pat: #ty = (#raw_ident).parse().map_err(|_| #parse_err)?;
            }
        })
        .collect()
}

/// Generate declarations and parsing logic for wrapper arguments.
pub(super) fn prepare_argument_processing(
    args: &ArgumentCollections<'_>,
    step_meta: StepMeta<'_>,
    ctx_ident: &proc_macro2::Ident,
    placeholder_names: &[syn::LitStr],
) -> PreparedArgs {
    let StepMeta { pattern, ident } = step_meta;
    let declares = gen_fixture_decls(args.fixtures, ident, ctx_ident);
    let captured: Vec<_> = args
        .step_args
        .iter()
        .enumerate()
        .map(|(idx, _)| {
            let index = syn::Index::from(idx);
            quote! { captures.get(#index).map(|m| m.as_str()) }
        })
        .collect();
    let all_captures: Vec<_> = placeholder_names
        .iter()
        .enumerate()
        .map(|(idx, _)| {
            let index = syn::Index::from(idx);
            quote! { captures.get(#index).map(|m| m.as_str()) }
        })
        .collect();
    let step_arg_parses = if args.step_struct.is_some() {
        Vec::new()
    } else {
        gen_step_parses(args.step_args, &captured, step_meta)
    };
    let step_struct_decl = gen_step_struct_decl(
        args.step_struct,
        &all_captures,
        placeholder_names,
        step_meta,
    );
    let datatable_decl = gen_datatable_decl(args.datatable, pattern, ident);
    let docstring_decl = gen_docstring_decl(args.docstring, pattern, ident);
    PreparedArgs {
        declares,
        step_arg_parses,
        step_struct_decl,
        datatable_decl,
        docstring_decl,
    }
}

/// Collect argument identifiers in the order declared by the step function.
pub(super) fn collect_ordered_arguments<'a>(
    call_order: &'a [CallArg],
    args: &ArgumentCollections<'a>,
) -> Vec<&'a syn::Ident> {
    call_order
        .iter()
        .map(|arg| match arg {
            CallArg::Fixture(i) =>
            {
                #[expect(
                    clippy::indexing_slicing,
                    reason = "call_order indices validated during macro expansion"
                )]
                &args.fixtures[*i].pat
            }
            CallArg::StepArg(i) =>
            {
                #[expect(
                    clippy::indexing_slicing,
                    reason = "call_order indices validated during macro expansion"
                )]
                &args.step_args[*i].pat
            }
            CallArg::StepStruct =>
            {
                #[expect(clippy::expect_used, reason = "variant guarantees presence")]
                &args
                    .step_struct
                    .expect("step struct present in call_order but not configured")
                    .pat
            }
            CallArg::DataTable =>
            {
                #[expect(clippy::expect_used, reason = "variant guarantees presence")]
                &args
                    .datatable
                    .expect("datatable present in call_order but not configured")
                    .pat
            }
            CallArg::DocString =>
            {
                #[expect(clippy::expect_used, reason = "variant guarantees presence")]
                &args
                    .docstring
                    .expect("docstring present in call_order but not configured")
                    .pat
            }
        })
        .collect()
}

#[cfg(test)]
mod tests;
