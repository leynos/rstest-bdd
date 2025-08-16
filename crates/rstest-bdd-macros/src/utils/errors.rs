//! Error handling helpers for macros.

use proc_macro2::TokenStream;

/// Convert a `syn::Error` into a `TokenStream` for macro errors.
pub(crate) fn error_to_tokens(err: &syn::Error) -> TokenStream {
    err.to_compile_error()
}
