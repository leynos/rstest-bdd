//! Async wrapper assembly for step wrapper emission.
//!
//! This module hosts the async wrapper generation path used for `async fn`
//! step definitions. The generated wrapper returns a `StepFuture` and uses a
//! future-aware unwind catcher to translate `skip!` and panics into the same
//! `StepExecution`/`StepError` flow as the synchronous wrappers.

use proc_macro2::TokenStream as TokenStream2;
use quote::{format_ident, quote};

use super::super::super::arguments::{
    PreparedArgs, StepMeta, collect_ordered_arguments, prepare_argument_processing,
};
use super::super::call_expr::generate_call_expression;
use super::super::identifiers::generate_wrapper_signature;

use super::{
    WrapperAssembly, WrapperErrors, WrapperIdentifiers, WrapperKind, WrapperLintConfig,
    WrapperRenderContext, generate_struct_assertion, prepare_wrapper_errors,
    process_datatable_cache, wrapper_expect_lint_paths,
};

/// Render an async wrapper function that awaits the user step.
fn render_async_wrapper_function(
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
    let expect_attr = super::generate_expect_attribute(&expect_lints);

    quote! {
        #expect_attr
        fn #wrapper_ident<'ctx>(
            #ctx_ident: &'ctx mut #path::StepContext<'_>,
            #text_ident: &'ctx str,
            docstring: Option<&'ctx str>,
            table: Option<&'ctx [&'ctx [&'ctx str]]>,
        ) -> #path::StepFuture<'ctx> {
            Box::pin(async move {
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

                match #path::__rstest_bdd_catch_unwind_future(async move { #call_expr }).await {
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
            })
        }
    }
}

/// Assemble the final async wrapper function using prepared components.
fn assemble_async_wrapper_function(
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
        wrapper_kind: WrapperKind::Async,
    };
    prepared.expect_lints = wrapper_expect_lint_paths(lint_config);
    render_async_wrapper_function(
        identifiers,
        prepared,
        WrapperRenderContext {
            errors,
            capture_count,
            call_expr: &call_expr,
        },
    )
}

/// Generate the async wrapper function body and pattern constant.
pub(super) fn generate_async_wrapper_body(
    config: &super::super::WrapperConfig<'_>,
    wrapper_ident: &proc_macro2::Ident,
    pattern_ident: &proc_macro2::Ident,
) -> TokenStream2 {
    let super::super::WrapperConfig {
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
    let wrapper_fn = assemble_async_wrapper_function(
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
        true,
    );

    quote! {
        #struct_assert
        #cache_tokens
        #signature
        #wrapper_fn
    }
}
