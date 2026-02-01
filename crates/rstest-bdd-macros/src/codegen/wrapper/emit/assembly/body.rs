//! Wrapper body emission helpers.
//!
//! This module contains the wrapper body generation logic that is shared
//! between sync and async wrapper emission, keeping `assembly.rs` focused on
//! orchestration and within the project's file length limits.

use super::super::super::args::ExtractedArgs;
use super::super::super::arguments::{
    StepMeta, collect_ordered_arguments, prepare_argument_processing,
};
use super::super::datatable_cache::{
    DatatableCacheComponents, generate_datatable_cache_definitions,
};
use super::super::identifiers::generate_wrapper_signature;
use proc_macro2::TokenStream as TokenStream2;
use quote::{format_ident, quote};

/// Generate a compile-time assertion that the step struct field count matches the
/// expected number of placeholder captures.
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

fn generate_wrapper_body_impl(
    config: &super::super::WrapperConfig<'_>,
    wrapper_ident: &proc_macro2::Ident,
    pattern_ident: &proc_macro2::Ident,
    wrapper_kind: super::WrapperKind,
) -> TokenStream2 {
    let super::super::WrapperConfig {
        ident,
        args,
        pattern,
        placeholder_names,
        placeholder_hints,
        capture_count,
        return_kind,
        is_async_step,
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
    let wrapper_fn = super::assemble_wrapper_function(
        super::WrapperIdentifiers {
            wrapper: wrapper_ident,
            pattern: pattern_ident,
            ctx: &ctx_ident,
            text: &text_ident,
        },
        super::WrapperAssembly {
            meta: step_meta,
            prepared,
            arg_idents,
            capture_count,
            return_kind,
        },
        wrapper_kind,
        is_async_step,
    );

    quote! {
        #struct_assert
        #cache_tokens
        #signature
        #wrapper_fn
    }
}

/// Generate the wrapper function body and pattern constant.
pub(super) fn generate_wrapper_body(
    config: &super::super::WrapperConfig<'_>,
    wrapper_ident: &proc_macro2::Ident,
    pattern_ident: &proc_macro2::Ident,
) -> TokenStream2 {
    generate_wrapper_body_impl(
        config,
        wrapper_ident,
        pattern_ident,
        super::WrapperKind::Sync,
    )
}

/// Generate the async variant of the wrapper function body and pattern constant.
///
/// This delegates to `generate_wrapper_body_impl`, selecting `super::WrapperKind::Async`
/// so the generated wrapper returns a `StepFuture` and awaits the step call path.
pub(super) fn generate_async_wrapper_body(
    config: &super::super::WrapperConfig<'_>,
    wrapper_ident: &proc_macro2::Ident,
    pattern_ident: &proc_macro2::Ident,
) -> TokenStream2 {
    generate_wrapper_body_impl(
        config,
        wrapper_ident,
        pattern_ident,
        super::WrapperKind::Async,
    )
}
