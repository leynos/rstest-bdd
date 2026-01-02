//! Wrapper body assembly for wrapper emission.

use super::super::args::ExtractedArgs;
use super::super::arguments::{
    PreparedArgs, StepMeta, collect_ordered_arguments, prepare_argument_processing,
};
use super::call_expr::generate_call_expression;
use super::datatable_cache::{DatatableCacheComponents, generate_datatable_cache_definitions};
use super::errors::{WrapperErrors, prepare_wrapper_errors};
use super::identifiers::generate_wrapper_signature;
use crate::return_classifier::ReturnKind;
use proc_macro2::TokenStream as TokenStream2;
use quote::{format_ident, quote};

/// Prepared wrapper inputs consumed by `assemble_wrapper_function`.
struct WrapperAssembly<'a> {
    meta: StepMeta<'a>,
    prepared: PreparedArgs,
    arg_idents: Vec<syn::Ident>,
    capture_count: usize,
    return_kind: ReturnKind,
}

/// Identifiers used during wrapper generation.
#[derive(Copy, Clone)]
struct WrapperIdentifiers<'a> {
    wrapper: &'a proc_macro2::Ident,
    pattern: &'a proc_macro2::Ident,
    ctx: &'a proc_macro2::Ident,
    text: &'a proc_macro2::Ident,
}

/// Assemble the final wrapper function using prepared components.
fn assemble_wrapper_function(
    identifiers: WrapperIdentifiers<'_>,
    assembly: WrapperAssembly<'_>,
) -> TokenStream2 {
    let WrapperIdentifiers {
        wrapper: wrapper_ident,
        pattern: pattern_ident,
        ctx: ctx_ident,
        text: text_ident,
    } = identifiers;
    let WrapperAssembly {
        meta,
        prepared,
        arg_idents,
        capture_count,
        return_kind,
    } = assembly;
    let PreparedArgs {
        declares,
        step_arg_parses,
        step_struct_decl,
        datatable_decl,
        docstring_decl,
    } = prepared;
    let WrapperErrors {
        placeholder: placeholder_err,
        panic: panic_err,
        execution: exec_err,
        capture_mismatch: capture_mismatch_err,
    } = prepare_wrapper_errors(meta, text_ident);
    let StepMeta { pattern: _, ident } = meta;
    let expected = capture_count;
    let path = crate::codegen::rstest_bdd_path();
    let call_expr = generate_call_expression(return_kind, ident, &arg_idents);
    quote! {
        fn #wrapper_ident(
            #ctx_ident: &mut #path::StepContext<'_>,
            #text_ident: &str,
            _docstring: Option<&str>,
            _table: Option<&[&[&str]]>,
        ) -> Result<#path::StepExecution, #path::StepError> {
            use std::panic::{catch_unwind, AssertUnwindSafe};
            let captures = #path::extract_placeholders(&#pattern_ident, #text_ident.into())
                .map_err(|e| #placeholder_err)?;
            let expected: usize = #expected;
            if captures.len() != expected {
                return Err(#capture_mismatch_err);
            }
            #(#declares)*
            #(#step_arg_parses)*
            #step_struct_decl
            #datatable_decl
            #docstring_decl
            match catch_unwind(AssertUnwindSafe(|| { #call_expr })) {
                Ok(res) => res
                    .map(|value| #path::StepExecution::from_value(value))
                    .map_err(|message| #exec_err),
                Err(payload) => match payload.downcast::<#path::SkipRequest>() {
                    Ok(skip) => Ok(#path::StepExecution::skipped(skip.into_message())),
                    Err(payload) => {
                        let message = #path::panic_message(payload.as_ref());
                        Err(#panic_err)
                    }
                },
            }
        }
    }
}

/// Generate the compile-time assertion for step struct field count.
fn generate_struct_assertion(args: &ExtractedArgs, capture_count: usize) -> Option<TokenStream2> {
    args.step_struct().map(|arg| {
        let ty = arg.ty;
        let path = crate::codegen::rstest_bdd_path();
        quote! {
            const _: [(); <#ty as #path::step_args::StepArgs>::FIELD_COUNT] = [(); #capture_count];
        }
    })
}

/// Generate datatable cache components and extract identifier references.
fn process_datatable_cache(
    args: &ExtractedArgs,
    wrapper_ident: &proc_macro2::Ident,
) -> (
    TokenStream2,
    Option<(proc_macro2::Ident, proc_macro2::Ident)>,
) {
    let cache_components =
        generate_datatable_cache_definitions(args.datatable().is_some(), wrapper_ident);
    match cache_components {
        DatatableCacheComponents::None => (proc_macro2::TokenStream::new(), None),
        DatatableCacheComponents::Some {
            tokens,
            key_ident,
            cache_ident,
        } => (tokens, Some((key_ident, cache_ident))),
    }
}

/// Generate the wrapper function body and pattern constant.
pub(super) fn generate_wrapper_body(
    config: &super::WrapperConfig<'_>,
    wrapper_ident: &proc_macro2::Ident,
    pattern_ident: &proc_macro2::Ident,
) -> TokenStream2 {
    let super::WrapperConfig {
        ident,
        args,
        pattern,
        placeholder_names,
        placeholder_hints,
        capture_count,
        return_kind,
        ..
    } = *config;

    let ctx_ident = format_ident!("__rstest_bdd_ctx");
    let text_ident = format_ident!("__rstest_bdd_text");
    let args_slice = &args.args;
    let step_meta = StepMeta { pattern, ident };
    let struct_assert = generate_struct_assertion(args, capture_count);
    let signature = generate_wrapper_signature(pattern, pattern_ident);
    let (cache_tokens, datatable_idents) = process_datatable_cache(args, wrapper_ident);
    let datatable_idents_refs = datatable_idents.as_ref().map(|(key, cache)| (key, cache));
    let prepared = prepare_argument_processing(
        args_slice,
        step_meta,
        &ctx_ident,
        placeholder_names,
        placeholder_hints,
        datatable_idents_refs,
    );
    let arg_idents = collect_ordered_arguments(args_slice);
    let wrapper_fn = assemble_wrapper_function(
        WrapperIdentifiers {
            wrapper: wrapper_ident,
            pattern: pattern_ident,
            ctx: &ctx_ident,
            text: &text_ident,
        },
        WrapperAssembly {
            meta: step_meta,
            prepared,
            arg_idents,
            capture_count,
            return_kind,
        },
    );

    quote! {
        #struct_assert
        #cache_tokens
        #signature
        #wrapper_fn
    }
}
