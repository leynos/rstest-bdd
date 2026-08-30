//! Wrapper body assembly for wrapper emission.
//!
//! This module combines prepared argument handling, error reporting, and the
//! call expression into a single wrapper body token stream. It keeps the
//! emission entry point focused on orchestration while centralizing the logic
//! that shapes the wrapper's structure.

use proc_macro2::TokenStream as TokenStream2;
use quote::quote;

use super::{
    super::arguments::{PreparedArgs, StepMeta},
    call_expr::generate_call_expression,
    errors::{WrapperErrors, prepare_wrapper_errors},
};
use crate::return_classifier::StepReturnStrategy;

mod async_wrapper;
mod body;
mod lint_config;

#[cfg(test)]
use lint_config::{
    LINT_NEEDLESS_PASS_BY_VALUE,
    LINT_REDUNDANT_CLOSURE,
    LINT_REDUNDANT_CLOSURE_FOR_METHOD_CALLS,
    LINT_SHADOW_REUSE,
    LINT_STR_TO_STRING,
    LINT_UNNECESSARY_WRAPS,
    WRAPPER_EXPECT_REASON,
};
use lint_config::{WrapperLintConfig, generate_expect_attribute, wrapper_expect_lint_paths};

/// Prepared wrapper inputs consumed by `assemble_wrapper_function`.
struct WrapperAssembly<'a> {
    /// Stores the internal `meta` value.
    meta: StepMeta<'a>,
    /// Stores the internal `prepared` value.
    prepared: PreparedArgs,
    /// Stores the internal `arg_idents` value.
    arg_idents: Vec<syn::Ident>,
    /// Stores the internal `capture_count` value.
    capture_count: usize,
    /// Stores the internal `strategy` value.
    strategy: StepReturnStrategy,
}

/// Identifiers used during wrapper generation.
#[derive(Copy, Clone)]
struct WrapperIdentifiers<'a> {
    /// Stores the internal `wrapper` value.
    wrapper: &'a proc_macro2::Ident,
    /// Stores the internal `pattern` value.
    pattern: &'a proc_macro2::Ident,
    /// Stores the internal `ctx` value.
    ctx: &'a proc_macro2::Ident,
    /// Stores the internal `text` value.
    text: &'a proc_macro2::Ident,
}

/// Context struct groups related render inputs.
struct WrapperRenderContext<'a> {
    /// Stores the internal `errors` value.
    errors: WrapperErrors,
    /// Stores the internal `capture_count` value.
    capture_count: usize,
    /// Stores the internal `call_expr` value.
    call_expr: &'a TokenStream2,
}

#[derive(Copy, Clone, PartialEq, Eq)]
/// Documents the internal `WrapperKind` item.
enum WrapperKind {
    /// Represents the internal validation outcome.
    Sync,
    /// Represents the internal validation outcome.
    Async,
}

/// Code fragments for wrapper function generation.
#[derive(Copy, Clone)]
struct WrapperCodeFragments<'a> {
    /// Stores the internal `path` value.
    path: &'a TokenStream2,
    /// Stores the internal `expect_attr` value.
    expect_attr: &'a TokenStream2,
    /// Stores the internal `capture_validation` value.
    capture_validation: &'a TokenStream2,
    /// Stores the internal `unwind_handling` value.
    unwind_handling: &'a TokenStream2,
}

/// Provides the internal `generate_sync_unwind_handling` operation.
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

/// Provides the internal `generate_sync_wrapper_quote` operation.
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

/// Provides the internal `generate_async_wrapper_quote` operation.
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
        strategy,
    } = assembly;
    let WrapperIdentifiers {
        text: text_ident, ..
    } = identifiers;
    let errors = prepare_wrapper_errors(meta, text_ident);
    let StepMeta { ident, .. } = meta;
    let call_expr = generate_call_expression(strategy, ident, &arg_idents, is_async_step);
    let lint_config = WrapperLintConfig {
        capture_count,
        has_step_struct: prepared.step_struct_decl.is_some(),
        has_step_arg_quote_strip: prepared.has_step_arg_quote_strip,
        strategy,
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
