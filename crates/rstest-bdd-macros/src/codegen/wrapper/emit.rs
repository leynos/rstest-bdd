//! Code emission helpers for wrapper generation.

use super::args::{Arg, ExtractedArgs};
use super::arguments::{
    PreparedArgs, StepMeta, collect_ordered_arguments, prepare_argument_processing,
};
use crate::return_classifier::ReturnKind;
use proc_macro2::TokenStream as TokenStream2;
use quote::{format_ident, quote};

mod call_expr;
mod datatable_cache;
mod errors;
mod identifiers;

use call_expr::generate_call_expression;
use datatable_cache::{DatatableCacheComponents, generate_datatable_cache_definitions};
use errors::{WrapperErrors, prepare_wrapper_errors};
use identifiers::{WrapperIdents, generate_wrapper_identifiers, next_wrapper_id};

/// Configuration required to generate a wrapper.
pub(crate) struct WrapperConfig<'a> {
    pub(crate) ident: &'a syn::Ident,
    pub(crate) args: &'a ExtractedArgs,
    pub(crate) pattern: &'a syn::LitStr,
    pub(crate) keyword: crate::StepKeyword,
    pub(crate) placeholder_names: &'a [syn::LitStr],
    /// Optional type hints for each placeholder, parallel to `placeholder_names`.
    pub(crate) placeholder_hints: &'a [Option<String>],
    pub(crate) capture_count: usize,
    pub(crate) return_kind: ReturnKind,
}

/// Generate the `StepPattern` constant used by a wrapper.
fn generate_wrapper_signature(
    pattern: &syn::LitStr,
    pattern_ident: &proc_macro2::Ident,
) -> TokenStream2 {
    let path = crate::codegen::rstest_bdd_path();
    quote! {
        static #pattern_ident: #path::StepPattern =
            #path::StepPattern::new(#pattern);
    }
}

/// Generate an async wrapper that wraps a sync step in an immediately-ready future.
///
/// This function produces a thin shim that calls the synchronous wrapper and wraps
/// its result using `std::future::ready`. The async wrapper enables sync step
/// definitions to participate in async scenario execution without modification.
fn generate_async_wrapper_from_sync(
    sync_wrapper_ident: &proc_macro2::Ident,
    async_wrapper_ident: &proc_macro2::Ident,
) -> TokenStream2 {
    let path = crate::codegen::rstest_bdd_path();
    quote! {
        fn #async_wrapper_ident<'a>(
            __rstest_bdd_ctx: &'a mut #path::StepContext<'a>,
            __rstest_bdd_text: &str,
            __rstest_bdd_docstring: Option<&str>,
            __rstest_bdd_table: Option<&[&[&str]]>,
        ) -> #path::StepFuture<'a> {
            Box::pin(::std::future::ready(
                #sync_wrapper_ident(
                    __rstest_bdd_ctx,
                    __rstest_bdd_text,
                    __rstest_bdd_docstring,
                    __rstest_bdd_table,
                )
            ))
        }
    }
}

/// Prepared wrapper inputs consumed by `assemble_wrapper_function`.
struct WrapperAssembly<'a> {
    meta: StepMeta<'a>,
    prepared: PreparedArgs,
    arg_idents: Vec<&'a syn::Ident>,
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
fn generate_wrapper_body(
    config: &WrapperConfig<'_>,
    wrapper_ident: &proc_macro2::Ident,
    pattern_ident: &proc_macro2::Ident,
) -> TokenStream2 {
    let WrapperConfig {
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

/// Generate fixture registration and inventory code for the wrapper.
fn generate_registration_code(
    config: &WrapperConfig<'_>,
    wrapper_idents: &WrapperIdents,
) -> TokenStream2 {
    let fixture_names: Vec<_> = config
        .args
        .fixtures()
        .map(|arg| {
            let Arg::Fixture { name, .. } = arg else {
                unreachable!("fixture iterator must only yield fixtures");
            };
            let rendered = name.to_string();
            quote! { #rendered }
        })
        .collect();
    let fixture_len = fixture_names.len();
    let keyword = config.keyword;
    let path = crate::codegen::rstest_bdd_path();
    let pattern_ident = &wrapper_idents.pattern_ident;
    let sync_wrapper_ident = &wrapper_idents.sync_wrapper;
    let async_wrapper_ident = &wrapper_idents.async_wrapper;
    let const_ident = &wrapper_idents.const_ident;
    quote! {
        const #const_ident: [&'static str; #fixture_len] = [#(#fixture_names),*];
        const _: [(); #fixture_len] = [(); #const_ident.len()];

        #path::step!(@pattern #keyword, &#pattern_ident, #sync_wrapper_ident, #async_wrapper_ident, &#const_ident);
    }
}

/// Generate the wrapper function and inventory registration.
///
/// This function generates both a synchronous wrapper and an async wrapper. The
/// async wrapper delegates to the sync wrapper, wrapping its result in an
/// immediately-ready future via `std::future::ready`.
pub(crate) fn generate_wrapper_code(config: &WrapperConfig<'_>) -> TokenStream2 {
    let id = next_wrapper_id();
    let wrapper_idents = generate_wrapper_identifiers(config.ident, id);
    let body = generate_wrapper_body(
        config,
        &wrapper_idents.sync_wrapper,
        &wrapper_idents.pattern_ident,
    );
    let async_wrapper_fn = generate_async_wrapper_from_sync(
        &wrapper_idents.sync_wrapper,
        &wrapper_idents.async_wrapper,
    );
    let registration = generate_registration_code(config, &wrapper_idents);

    quote! {
        #body
        #async_wrapper_fn
        #registration
    }
}

#[cfg(test)]
mod tests;
