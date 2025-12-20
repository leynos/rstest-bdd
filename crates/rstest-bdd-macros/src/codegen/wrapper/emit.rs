//! Code emission helpers for wrapper generation.
// TODO(issue #50): Reduce this module below 400 lines and remove the rs-length allowlist entry.

use super::args::{Arg, ExtractedArgs};
use super::arguments::{
    PreparedArgs, StepMeta, collect_ordered_arguments, prepare_argument_processing,
    step_error_tokens,
};
use crate::return_classifier::ReturnKind;
use crate::utils::ident::sanitize_ident;
use proc_macro2::TokenStream as TokenStream2;
use quote::{format_ident, quote};
use std::sync::atomic::{AtomicUsize, Ordering};

mod datatable_cache;
use datatable_cache::{DatatableCacheComponents, generate_datatable_cache_definitions};

/// Configuration required to generate a wrapper.
pub(crate) struct WrapperConfig<'a> {
    pub(crate) ident: &'a syn::Ident,
    pub(crate) args: &'a ExtractedArgs,
    pub(crate) pattern: &'a syn::LitStr,
    pub(crate) keyword: crate::StepKeyword,
    pub(crate) placeholder_names: &'a [syn::LitStr],
    pub(crate) capture_count: usize,
    pub(crate) return_kind: ReturnKind,
}

static COUNTER: AtomicUsize = AtomicUsize::new(0);

/// Generate unique identifiers for the wrapper components.
///
/// The provided step function identifier may contain Unicode. It is
/// sanitized to ASCII before constructing constant names to avoid emitting
/// invalid identifiers.
///
/// Returns identifiers for the wrapper function, fixture array constant, and
/// pattern constant.
fn generate_wrapper_identifiers(
    ident: &syn::Ident,
    id: usize,
) -> (proc_macro2::Ident, proc_macro2::Ident, proc_macro2::Ident) {
    let ident_sanitized = sanitize_ident(&ident.to_string());
    let wrapper_ident = format_ident!("__rstest_bdd_wrapper_{}_{}", ident_sanitized, id);
    let ident_upper = ident_sanitized.to_ascii_uppercase();
    let const_ident = format_ident!("__RSTEST_BDD_FIXTURES_{}_{}", ident_upper, id);
    let pattern_ident = format_ident!("__RSTEST_BDD_PATTERN_{}_{}", ident_upper, id);
    (wrapper_ident, const_ident, pattern_ident)
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

struct WrapperErrors {
    placeholder: TokenStream2,
    panic: TokenStream2,
    execution: TokenStream2,
    capture_mismatch: TokenStream2,
}

fn prepare_wrapper_errors(meta: StepMeta<'_>, text_ident: &proc_macro2::Ident) -> WrapperErrors {
    let StepMeta { pattern, ident } = meta;
    let execution_error = format_ident!("ExecutionError");
    let panic_error = format_ident!("PanicError");
    let placeholder = step_error_tokens(
        &execution_error,
        pattern,
        ident,
        &quote! {
            format!(
                "Step text '{}' does not match pattern '{}': {}",
                #text_ident,
                #pattern,
                e
            )
        },
    );
    let panic = step_error_tokens(&panic_error, pattern, ident, &quote! { message });
    let execution = step_error_tokens(&execution_error, pattern, ident, &quote! { message });
    let capture_mismatch = step_error_tokens(
        &execution_error,
        pattern,
        ident,
        &quote! {
            format!(
                "pattern '{}' produced {} captures but step '{}' expects {}",
                #pattern,
                captures.len(),
                stringify!(#ident),
                expected
            )
        },
    );

    WrapperErrors {
        placeholder,
        panic,
        execution,
        capture_mismatch,
    }
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
    let call = quote! { #ident(#(#arg_idents),*) };
    let call_expr = match return_kind {
        ReturnKind::Unit => quote! {{
            #call;
            Ok(None)
        }},
        ReturnKind::Value => quote! {
            Ok(Some(Box::new(#call) as Box<dyn std::any::Any>))
        },
        ReturnKind::ResultUnit | ReturnKind::ResultValue => quote! {
            #call
                .map(|value| #path::__rstest_bdd_payload_from_value(value))
                .map_err(|e| e.to_string())
        },
    };

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
        capture_count,
        return_kind,
        ..
    } = *config;

    let ctx_ident = format_ident!("__rstest_bdd_ctx");
    let text_ident = format_ident!("__rstest_bdd_text");
    let args_slice = &args.args;
    let step_meta = StepMeta { pattern, ident };
    let struct_assert = args.step_struct().map(|arg| {
        let ty = arg.ty;
        let count = capture_count;
        let path = crate::codegen::rstest_bdd_path();
        quote! {
            const _: [(); <#ty as #path::step_args::StepArgs>::FIELD_COUNT] = [(); #count];
        }
    });
    let signature = generate_wrapper_signature(pattern, pattern_ident);
    let cache_components =
        generate_datatable_cache_definitions(args.datatable().is_some(), wrapper_ident);
    let (cache_tokens, datatable_idents) = match cache_components {
        DatatableCacheComponents::None => (proc_macro2::TokenStream::new(), None),
        DatatableCacheComponents::Some {
            tokens,
            key_ident,
            cache_ident,
        } => (tokens, Some((key_ident, cache_ident))),
    };
    let datatable_idents_refs = datatable_idents.as_ref().map(|(key, cache)| (key, cache));
    let prepared = prepare_argument_processing(
        args_slice,
        step_meta,
        &ctx_ident,
        placeholder_names,
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
    pattern_ident: &proc_macro2::Ident,
    wrapper_ident: &proc_macro2::Ident,
    const_ident: &proc_macro2::Ident,
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
    quote! {
        const #const_ident: [&'static str; #fixture_len] = [#(#fixture_names),*];
        const _: [(); #fixture_len] = [(); #const_ident.len()];

        #path::step!(@pattern #keyword, &#pattern_ident, #wrapper_ident, &#const_ident);
    }
}

/// Generate the wrapper function and inventory registration.
pub(crate) fn generate_wrapper_code(config: &WrapperConfig<'_>) -> TokenStream2 {
    // Relaxed ordering suffices: the counter only ensures a unique suffix and
    // is not used for synchronisation with other data.
    let id = COUNTER.fetch_add(1, Ordering::Relaxed);
    let (wrapper_ident, const_ident, pattern_ident) =
        generate_wrapper_identifiers(config.ident, id);
    let body = generate_wrapper_body(config, &wrapper_ident, &pattern_ident);
    let registration =
        generate_registration_code(config, &pattern_ident, &wrapper_ident, &const_ident);

    quote! {
        #body
        #registration
    }
}

#[cfg(test)]
mod tests;
