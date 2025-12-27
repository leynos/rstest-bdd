//! Error token generation for wrapper functions.

use super::super::arguments::{StepMeta, step_error_tokens};
use proc_macro2::TokenStream as TokenStream2;
use quote::{format_ident, quote};

/// Pre-generated error tokens for wrapper function error paths.
pub(super) struct WrapperErrors {
    pub placeholder: TokenStream2,
    pub panic: TokenStream2,
    pub execution: TokenStream2,
    pub capture_mismatch: TokenStream2,
}

/// Generate all error token streams for a wrapper function.
pub(super) fn prepare_wrapper_errors(
    meta: StepMeta<'_>,
    text_ident: &proc_macro2::Ident,
) -> WrapperErrors {
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
