//! Wrapper body assembly for wrapper emission.
//!
//! This module combines prepared argument handling, error reporting, and the
//! call expression into a single wrapper body token stream. It keeps the
//! emission entry point focused on orchestration while centralising the logic
//! that shapes the wrapper's structure.

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

#[path = "assembly/async_wrapper.rs"]
mod async_wrapper;

const WRAPPER_EXPECT_REASON: &str = "rstest-bdd step wrapper pattern requires these patterns \
for parameter extraction, Result normalization, and closure-based error handling";
const LINT_SHADOW_REUSE: &str = "clippy::shadow_reuse";
const LINT_UNNECESSARY_WRAPS: &str = "clippy::unnecessary_wraps";
const LINT_STR_TO_STRING: &str = "clippy::str_to_string";
const LINT_REDUNDANT_CLOSURE_FOR_METHOD_CALLS: &str = "clippy::redundant_closure_for_method_calls";
const LINT_NEEDLESS_PASS_BY_VALUE: &str = "clippy::needless_pass_by_value";
const LINT_REDUNDANT_CLOSURE: &str = "clippy::redundant_closure";
const LINT_NEEDLESS_LIFETIMES: &str = "clippy::needless_lifetimes";

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

/// Context struct groups related render inputs.
struct WrapperRenderContext<'a> {
    errors: WrapperErrors,
    capture_count: usize,
    call_expr: &'a TokenStream2,
}

#[derive(Copy, Clone, PartialEq, Eq)]
enum WrapperKind {
    Sync,
    Async,
}

#[derive(Copy, Clone)]
struct WrapperLintConfig {
    capture_count: usize,
    has_step_struct: bool,
    has_step_arg_quote_strip: bool,
    return_kind: ReturnKind,
    wrapper_kind: WrapperKind,
}

fn wrapper_expect_lint_names(config: WrapperLintConfig) -> Vec<&'static str> {
    let mut lints = Vec::new();
    if config.has_step_arg_quote_strip {
        lints.push(LINT_SHADOW_REUSE);
    }
    if matches!(config.return_kind, ReturnKind::Unit | ReturnKind::Value) {
        lints.push(LINT_UNNECESSARY_WRAPS);
    }
    let has_placeholders = config.capture_count > 0;
    if config.has_step_struct && has_placeholders {
        lints.push(LINT_STR_TO_STRING);
    }
    if has_placeholders {
        lints.push(LINT_REDUNDANT_CLOSURE_FOR_METHOD_CALLS);
    }
    if config.wrapper_kind == WrapperKind::Async {
        lints.push(LINT_NEEDLESS_LIFETIMES);
    }
    lints.push(LINT_NEEDLESS_PASS_BY_VALUE);
    lints.push(LINT_REDUNDANT_CLOSURE);
    lints
}

fn lint_path_from_str(lint: &str) -> syn::Path {
    let mut segments = syn::punctuated::Punctuated::new();
    for segment in lint.split("::") {
        let ident = syn::Ident::new(segment, proc_macro2::Span::call_site());
        segments.push(syn::PathSegment::from(ident));
    }
    syn::Path {
        leading_colon: None,
        segments,
    }
}

fn wrapper_expect_lint_paths(config: WrapperLintConfig) -> Vec<syn::Path> {
    wrapper_expect_lint_names(config)
        .iter()
        .map(|lint| lint_path_from_str(lint))
        .collect()
}

/// Generate the expect attribute for suppressing known Clippy lints in wrapper functions.
fn generate_expect_attribute(lint_paths: &[syn::Path]) -> TokenStream2 {
    if lint_paths.is_empty() {
        return TokenStream2::new();
    }
    quote! {
        #[expect(
            #(#lint_paths,)*
            reason = #WRAPPER_EXPECT_REASON
        )]
    }
}

/// Render the wrapper function tokens from prepared inputs.
fn render_wrapper_function(
    identifiers: WrapperIdentifiers<'_>,
    prepared: PreparedArgs,
    context: WrapperRenderContext<'_>,
) -> TokenStream2 {
    let WrapperIdentifiers {
        wrapper: wrapper_ident,
        pattern: pattern_ident,
        ctx: ctx_ident,
        text: text_ident,
    } = identifiers;
    let PreparedArgs {
        declares,
        step_arg_parses,
        step_struct_decl,
        datatable_decl,
        docstring_decl,
        expect_lints,
        ..
    } = prepared;
    let WrapperRenderContext {
        errors,
        capture_count,
        call_expr,
    } = context;
    let WrapperErrors {
        placeholder: placeholder_err,
        panic: panic_err,
        execution: exec_err,
        capture_mismatch: capture_mismatch_err,
    } = errors;
    let expected = capture_count;
    let path = crate::codegen::rstest_bdd_path();
    let expect_attr = generate_expect_attribute(&expect_lints);
    quote! {
        #expect_attr
        fn #wrapper_ident(
            #ctx_ident: &mut #path::StepContext<'_>,
            #text_ident: &str,
            docstring: Option<&str>,
            table: Option<&[&[&str]]>,
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

/// Assemble the final wrapper function using prepared components.
fn assemble_wrapper_function(
    identifiers: WrapperIdentifiers<'_>,
    assembly: WrapperAssembly<'_>,
    is_async_step: bool,
) -> TokenStream2 {
    let WrapperAssembly {
        meta,
        mut prepared,
        arg_idents,
        capture_count,
        return_kind,
    } = assembly;
    let WrapperIdentifiers {
        text: text_ident, ..
    } = identifiers;
    let errors = prepare_wrapper_errors(meta, text_ident);
    let StepMeta { ident, .. } = meta;
    let call_expr = generate_call_expression(return_kind, ident, &arg_idents, is_async_step);
    let lint_config = WrapperLintConfig {
        capture_count,
        has_step_struct: prepared.step_struct_decl.is_some(),
        has_step_arg_quote_strip: prepared.has_step_arg_quote_strip,
        return_kind,
        wrapper_kind: WrapperKind::Sync,
    };
    prepared.expect_lints = wrapper_expect_lint_paths(lint_config);
    render_wrapper_function(
        identifiers,
        prepared,
        WrapperRenderContext {
            errors,
            capture_count,
            call_expr: &call_expr,
        },
    )
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
        false,
    );

    quote! {
        #struct_assert
        #cache_tokens
        #signature
        #wrapper_fn
    }
}

pub(super) fn generate_async_wrapper_body(
    config: &super::WrapperConfig<'_>,
    wrapper_ident: &proc_macro2::Ident,
    pattern_ident: &proc_macro2::Ident,
) -> TokenStream2 {
    async_wrapper::generate_async_wrapper_body(config, wrapper_ident, pattern_ident)
}

#[cfg(test)]
mod tests;
