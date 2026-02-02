//! Code emission helpers for wrapper generation.

use super::args::{Arg, ExtractedArgs};
use crate::return_classifier::ReturnKind;
use proc_macro2::TokenStream as TokenStream2;
use quote::quote;

mod assembly;
mod call_expr;
mod datatable_cache;
mod errors;
mod identifiers;

use assembly::{generate_async_wrapper_body, generate_wrapper_body};
use identifiers::{WrapperIdents, generate_wrapper_identifiers, next_wrapper_id};

/// Configuration required to generate a wrapper.
pub(crate) struct WrapperConfig<'a> {
    pub(crate) ident: &'a syn::Ident,
    pub(crate) is_async_step: bool,
    pub(crate) args: &'a ExtractedArgs,
    pub(crate) pattern: &'a syn::LitStr,
    pub(crate) keyword: crate::StepKeyword,
    pub(crate) placeholder_names: &'a [syn::LitStr],
    /// Optional type hints for each placeholder, parallel to `placeholder_names`.
    pub(crate) placeholder_hints: &'a [Option<String>],
    pub(crate) capture_count: usize,
    pub(crate) return_kind: ReturnKind,
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
        fn #async_wrapper_ident<'ctx, 'fixtures>(
            __rstest_bdd_ctx: &'ctx mut #path::StepContext<'fixtures>,
            __rstest_bdd_text: &'ctx str,
            __rstest_bdd_docstring: Option<&'ctx str>,
            __rstest_bdd_table: Option<&'ctx [&'ctx [&'ctx str]]>,
        ) -> #path::StepFuture<'ctx> {
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

/// Generate a sync wrapper for an async step by blocking on the async wrapper.
///
/// This supports executing async-only steps from synchronous scenarios.
fn generate_sync_wrapper_from_async(
    config: &WrapperConfig<'_>,
    sync_wrapper_ident: &proc_macro2::Ident,
    async_wrapper_ident: &proc_macro2::Ident,
) -> TokenStream2 {
    let path = crate::codegen::rstest_bdd_path();
    let pattern = config.pattern;
    let ident = config.ident;
    quote! {
        fn #sync_wrapper_ident(
            __rstest_bdd_ctx: &mut #path::StepContext<'_>,
            __rstest_bdd_text: &str,
            __rstest_bdd_docstring: Option<&str>,
            __rstest_bdd_table: Option<&[&[&str]]>,
        ) -> Result<#path::StepExecution, #path::StepError> {
            if ::tokio::runtime::Handle::try_current().is_ok() {
                return Err(#path::StepError::ExecutionError {
                    pattern: #pattern.to_string(),
                    function: stringify!(#ident).to_string(),
                    message: concat!(
                        "async step executed via sync handler while a Tokio runtime is running; ",
                        "run the scenario with `runtime = \"tokio-current-thread\"` or make the scenario test `async fn`",
                    ).to_string(),
                });
            }

            let runtime = ::tokio::runtime::Builder::new_current_thread()
                .enable_all()
                .build()
                .map_err(|e| #path::StepError::ExecutionError {
                    pattern: #pattern.to_string(),
                    function: stringify!(#ident).to_string(),
                    message: format!("failed to construct Tokio current-thread runtime: {e}"),
                })?;

            let local_set = ::tokio::task::LocalSet::new();
            local_set.block_on(
                &runtime,
                #async_wrapper_ident(
                    __rstest_bdd_ctx,
                    __rstest_bdd_text,
                    __rstest_bdd_docstring,
                    __rstest_bdd_table,
                ),
            )
        }
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
    let execution_mode = if config.is_async_step {
        quote! { #path::StepExecutionMode::Async }
    } else {
        quote! { #path::StepExecutionMode::Both }
    };
    quote! {
        const #const_ident: [&'static str; #fixture_len] = [#(#fixture_names),*];
        const _: [(); #fixture_len] = [(); #const_ident.len()];

        #path::step!(
            @pattern #keyword,
            &#pattern_ident,
            #sync_wrapper_ident,
            #async_wrapper_ident,
            &#const_ident,
            #execution_mode
        );
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
    let body = if config.is_async_step {
        generate_async_wrapper_body(
            config,
            &wrapper_idents.async_wrapper,
            &wrapper_idents.pattern_ident,
        )
    } else {
        generate_wrapper_body(
            config,
            &wrapper_idents.sync_wrapper,
            &wrapper_idents.pattern_ident,
        )
    };
    let async_wrapper_fn = if config.is_async_step {
        generate_sync_wrapper_from_async(
            config,
            &wrapper_idents.sync_wrapper,
            &wrapper_idents.async_wrapper,
        )
    } else {
        generate_async_wrapper_from_sync(
            &wrapper_idents.sync_wrapper,
            &wrapper_idents.async_wrapper,
        )
    };
    let registration = generate_registration_code(config, &wrapper_idents);

    quote! {
        #body
        #async_wrapper_fn
        #registration
    }
}

#[cfg(test)]
mod tests;
