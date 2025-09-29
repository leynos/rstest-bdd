//! Argument code generation utilities shared by wrapper emission logic.

use super::args::{ArgumentCollections, CallArg, DataTableArg, DocStringArg, FixtureArg, StepArg};
use proc_macro2::TokenStream as TokenStream2;
use quote::{format_ident, quote};

#[derive(Copy, Clone)]
pub(super) struct StepMeta<'a> {
    pub(super) pattern: &'a syn::LitStr,
    pub(super) ident: &'a syn::Ident,
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

/// Generate declarations for fixture values.
///
/// Non-reference fixtures must implement [`Clone`] because wrappers clone
/// them to hand ownership to the step function.
pub(super) fn gen_fixture_decls(fixtures: &[FixtureArg], ident: &syn::Ident) -> Vec<TokenStream2> {
    fixtures
        .iter()
        .map(|FixtureArg { pat, name, ty }| {
            let path = crate::codegen::rstest_bdd_path();
            let lookup_ty = if let syn::Type::Reference(r) = ty {
                &*r.elem
            } else {
                ty
            };
            let clone_suffix = if matches!(ty, syn::Type::Reference(_)) {
                quote! {}
            } else {
                quote! { .cloned() }
            };
            quote! {
                let #pat: #ty = ctx
                    .get::<#lookup_ty>(stringify!(#name))
                    #clone_suffix
                    .ok_or_else(|| #path::StepError::MissingFixture {
                        name: stringify!(#name).to_string(),
                        ty: stringify!(#lookup_ty).to_string(),
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
) -> (
    Vec<TokenStream2>,
    Vec<TokenStream2>,
    Option<TokenStream2>,
    Option<TokenStream2>,
) {
    let StepMeta { pattern, ident } = step_meta;
    let declares = gen_fixture_decls(args.fixtures, ident);
    let captured: Vec<_> = args
        .step_args
        .iter()
        .enumerate()
        .map(|(idx, _)| {
            let index = syn::Index::from(idx);
            quote! { captures.get(#index).map(|m| m.as_str()) }
        })
        .collect();
    let step_arg_parses = gen_step_parses(args.step_args, &captured, step_meta);
    let datatable_decl = gen_datatable_decl(args.datatable, pattern, ident);
    let docstring_decl = gen_docstring_decl(args.docstring, pattern, ident);
    (declares, step_arg_parses, datatable_decl, docstring_decl)
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
mod tests {
    //! Tests for argument preparation helpers.

    use super::*;
    use crate::codegen::wrapper::args::{
        ArgumentCollections, CallArg, DataTableArg, DocStringArg, FixtureArg, StepArg,
    };
    use quote::quote;
    use syn::parse_quote;

    fn sample_meta<'a>(pattern: &'a syn::LitStr, ident: &'a syn::Ident) -> StepMeta<'a> {
        StepMeta { pattern, ident }
    }

    fn build_arguments() -> (Vec<FixtureArg>, Vec<StepArg>, DataTableArg, DocStringArg) {
        let fixtures = vec![FixtureArg {
            pat: parse_quote!(db),
            name: parse_quote!(db),
            ty: parse_quote!(String),
        }];
        let step_args = vec![StepArg {
            pat: parse_quote!(count),
            ty: parse_quote!(usize),
        }];
        let datatable = DataTableArg {
            pat: parse_quote!(table),
            ty: parse_quote!(Vec<Vec<String>>),
        };
        let docstring = DocStringArg {
            pat: parse_quote!(doc),
        };
        (fixtures, step_args, datatable, docstring)
    }

    #[test]
    fn prepare_argument_processing_handles_all_argument_types() {
        let (fixtures, step_args, datatable, docstring) = build_arguments();
        let collections = ArgumentCollections {
            fixtures: &fixtures,
            step_args: &step_args,
            datatable: Some(&datatable),
            docstring: Some(&docstring),
        };
        let pattern: syn::LitStr = parse_quote!("^pattern$");
        let ident: syn::Ident = parse_quote!(demo_step);
        let meta = sample_meta(&pattern, &ident);

        let (fixture_decls, step_parses, datatable_decl, docstring_decl) =
            prepare_argument_processing(&collections, meta);

        assert_eq!(fixture_decls.len(), 1);
        let [fixture_stmt] = fixture_decls.as_slice() else {
            panic!("expected single fixture declaration");
        };
        let fixture_code = fixture_stmt.to_string();
        assert!(fixture_code.contains("ctx"));
        assert!(fixture_code.contains("cloned"));
        assert!(fixture_code.contains("MissingFixture"));

        assert_eq!(step_parses.len(), 1);
        let [parse_stmt] = step_parses.as_slice() else {
            panic!("expected single step argument parser");
        };
        let parse_code = parse_stmt.to_string();
        assert!(parse_code.contains("captures"));
        assert!(parse_code.contains("parse"));

        let Some(datatable_code) = datatable_decl else {
            panic!("expected datatable declaration");
        };
        assert!(datatable_code.to_string().contains("iter"));

        let Some(docstring_code) = docstring_decl else {
            panic!("expected docstring declaration");
        };
        assert!(docstring_code.to_string().contains("to_owned"));
    }

    #[test]
    fn collect_ordered_arguments_preserves_call_order() {
        let (fixtures, step_args, datatable, docstring) = build_arguments();
        let collections = ArgumentCollections {
            fixtures: &fixtures,
            step_args: &step_args,
            datatable: Some(&datatable),
            docstring: Some(&docstring),
        };
        let order = [
            CallArg::Fixture(0),
            CallArg::StepArg(0),
            CallArg::DataTable,
            CallArg::DocString,
        ];
        let names: Vec<String> = collect_ordered_arguments(&order, &collections)
            .into_iter()
            .map(std::string::ToString::to_string)
            .collect();

        assert_eq!(names, ["db", "count", "table", "doc"]);
    }

    #[test]
    fn gen_fixture_decls_omits_clone_for_reference_types() {
        let fixtures = vec![
            FixtureArg {
                pat: parse_quote!(owned_fixture),
                name: parse_quote!(owned_fixture),
                ty: parse_quote!(String),
            },
            FixtureArg {
                pat: parse_quote!(ref_fixture),
                name: parse_quote!(ref_fixture),
                ty: parse_quote!(&'static str),
            },
        ];
        let ident: syn::Ident = parse_quote!(step_fn);
        let tokens = gen_fixture_decls(&fixtures, &ident);
        let [owned, borrowed] = tokens.as_slice() else {
            panic!("expected two fixture declarations");
        };

        assert!(owned.to_string().contains("cloned"));
        assert!(!borrowed.to_string().contains("cloned"));
    }

    #[test]
    fn step_error_tokens_embed_variant_and_message() {
        let variant: syn::Ident = parse_quote!(ExecutionError);
        let pattern: syn::LitStr = parse_quote!("pattern");
        let ident: syn::Ident = parse_quote!(step_fn);
        let message = quote! { "failure".to_string() };

        let tokens = step_error_tokens(&variant, &pattern, &ident, &message).to_string();

        assert!(tokens.contains("StepError :: ExecutionError"));
        assert!(tokens.contains("pattern :"));
        assert!(tokens.contains("function :"));
        assert!(tokens.contains("message :"));
        assert!(tokens.contains(r#""failure""#));
    }
}
