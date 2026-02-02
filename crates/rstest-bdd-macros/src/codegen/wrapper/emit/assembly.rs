//! Wrapper body assembly for wrapper emission.
//!
//! This module combines prepared argument handling, error reporting, and the
//! call expression into a single wrapper body token stream. It keeps the
//! emission entry point focused on orchestration while centralizing the logic
//! that shapes the wrapper's structure.

use super::super::arguments::PreparedArgs;
use super::super::arguments::StepMeta;
use super::call_expr::generate_call_expression;
use super::errors::{WrapperErrors, prepare_wrapper_errors};
use crate::return_classifier::ReturnKind;
use proc_macro2::TokenStream as TokenStream2;
use quote::quote;

#[path = "assembly/async_wrapper.rs"]
mod async_wrapper;

#[path = "assembly/body.rs"]
mod body;

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

/// Code fragments for wrapper function generation.
#[derive(Copy, Clone)]
struct WrapperCodeFragments<'a> {
    path: &'a TokenStream2,
    expect_attr: &'a TokenStream2,
    capture_validation: &'a TokenStream2,
    unwind_handling: &'a TokenStream2,
}

fn generate_sync_unwind_handling(
    path: &TokenStream2,
    call_expr: &TokenStream2,
    exec_err: &TokenStream2,
    panic_err: &TokenStream2,
) -> TokenStream2 {
    quote! {
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

/// Generate the common body components shared by sync and async wrappers.
fn generate_wrapper_body_tokens(
    capture_validation: &TokenStream2,
    prepared: &PreparedArgs,
) -> TokenStream2 {
    let declares = &prepared.declares;
    let step_arg_parses = &prepared.step_arg_parses;
    let step_struct_decl = &prepared.step_struct_decl;
    let datatable_decl = &prepared.datatable_decl;
    let docstring_decl = &prepared.docstring_decl;

    quote! {
        #capture_validation
        #(#declares)*
        #(#step_arg_parses)*
        #step_struct_decl
        #datatable_decl
        #docstring_decl
    }
}

fn generate_sync_wrapper_quote(
    identifiers: WrapperIdentifiers<'_>,
    prepared: &PreparedArgs,
    fragments: WrapperCodeFragments<'_>,
) -> TokenStream2 {
    let WrapperCodeFragments {
        path,
        expect_attr,
        capture_validation,
        unwind_handling,
    } = fragments;
    let WrapperIdentifiers {
        wrapper: wrapper_ident,
        ctx: ctx_ident,
        text: text_ident,
        ..
    } = identifiers;
    let body_tokens = generate_wrapper_body_tokens(capture_validation, prepared);

    quote! {
        #expect_attr
        fn #wrapper_ident(
            #ctx_ident: &mut #path::StepContext<'_>,
            #text_ident: &str,
            docstring: Option<&str>,
            table: Option<&[&[&str]]>,
        ) -> Result<#path::StepExecution, #path::StepError> {
            use std::panic::{catch_unwind, AssertUnwindSafe};
            #body_tokens
            #unwind_handling
        }
    }
}

fn generate_async_wrapper_quote(
    identifiers: WrapperIdentifiers<'_>,
    prepared: &PreparedArgs,
    fragments: WrapperCodeFragments<'_>,
) -> TokenStream2 {
    let WrapperCodeFragments {
        path,
        expect_attr,
        capture_validation,
        unwind_handling,
    } = fragments;
    let WrapperIdentifiers {
        wrapper: wrapper_ident,
        ctx: ctx_ident,
        text: text_ident,
        ..
    } = identifiers;
    let body_tokens = generate_wrapper_body_tokens(capture_validation, prepared);

    quote! {
        #expect_attr
        fn #wrapper_ident<'ctx>(
            #ctx_ident: &'ctx mut #path::StepContext<'_>,
            #text_ident: &'ctx str,
            docstring: Option<&'ctx str>,
            table: Option<&'ctx [&'ctx [&'ctx str]]>,
        ) -> #path::StepFuture<'ctx> {
            Box::pin(async move {
                #body_tokens
                #unwind_handling
            })
        }
    }
}

/// Render wrapper function tokens from prepared inputs.
///
/// The wrapper kind controls whether the generated function is synchronous or
/// returns a boxed future for `async fn` step definitions.
fn render_wrapper_function(
    identifiers: WrapperIdentifiers<'_>,
    prepared: &PreparedArgs,
    context: WrapperRenderContext<'_>,
    wrapper_kind: WrapperKind,
) -> TokenStream2 {
    let WrapperIdentifiers {
        pattern: pattern_ident,
        text: text_ident,
        ..
    } = identifiers;
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
    let path = crate::codegen::rstest_bdd_path();
    let expect_attr = generate_expect_attribute(&prepared.expect_lints);

    let capture_validation = async_wrapper::generate_capture_validation(
        &path,
        async_wrapper::CaptureValidationIdentifiers {
            pattern: pattern_ident,
            text: text_ident,
        },
        capture_count,
        async_wrapper::CaptureValidationErrors {
            placeholder: &placeholder_err,
            capture_mismatch: &capture_mismatch_err,
        },
    );

    match wrapper_kind {
        WrapperKind::Sync => {
            let unwind_handling =
                generate_sync_unwind_handling(&path, call_expr, &exec_err, &panic_err);
            let fragments = WrapperCodeFragments {
                path: &path,
                expect_attr: &expect_attr,
                capture_validation: &capture_validation,
                unwind_handling: &unwind_handling,
            };
            generate_sync_wrapper_quote(identifiers, prepared, fragments)
        }
        WrapperKind::Async => {
            let unwind_handling =
                async_wrapper::generate_unwind_handling(&path, call_expr, &exec_err, &panic_err);
            let fragments = WrapperCodeFragments {
                path: &path,
                expect_attr: &expect_attr,
                capture_validation: &capture_validation,
                unwind_handling: &unwind_handling,
            };
            generate_async_wrapper_quote(identifiers, prepared, fragments)
        }
    }
}

/// Assemble the final wrapper function using prepared components.
fn assemble_wrapper_function(
    identifiers: WrapperIdentifiers<'_>,
    assembly: WrapperAssembly<'_>,
    wrapper_kind: WrapperKind,
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
        wrapper_kind,
    };
    prepared.expect_lints = wrapper_expect_lint_paths(lint_config);
    render_wrapper_function(
        identifiers,
        &prepared,
        WrapperRenderContext {
            errors,
            capture_count,
            call_expr: &call_expr,
        },
        wrapper_kind,
    )
}

/// Generate the wrapper function body and pattern constant.
pub(super) fn generate_wrapper_body(
    config: &super::WrapperConfig<'_>,
    wrapper_ident: &proc_macro2::Ident,
    pattern_ident: &proc_macro2::Ident,
) -> TokenStream2 {
    body::generate_wrapper_body(config, wrapper_ident, pattern_ident)
}

/// Generate the wrapper function body and pattern constant for async wrappers.
///
/// Parameters:
/// - `config`: wrapper configuration and extracted step metadata
/// - `wrapper_ident`: identifier for the generated wrapper function
/// - `pattern_ident`: identifier for the generated pattern constant
///
/// Returns the generated tokens as a `TokenStream2`.
pub(super) fn generate_async_wrapper_body(
    config: &super::WrapperConfig<'_>,
    wrapper_ident: &proc_macro2::Ident,
    pattern_ident: &proc_macro2::Ident,
) -> TokenStream2 {
    body::generate_async_wrapper_body(config, wrapper_ident, pattern_ident)
}

#[cfg(test)]
mod tests;
