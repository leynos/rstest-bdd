//! Error handling helpers for macros.

use proc_macro2::TokenStream;

/// Convert a `syn::Error` into a `TokenStream` for macro errors.
pub(crate) fn error_to_tokens(err: &syn::Error) -> TokenStream {
    err.to_compile_error()
}

/// Produce a deterministic message when reading a directory fails.
///
/// Normalises platform-specific `NotFound` errors so tests see a consistent
/// string regardless of operating system.
///
/// # Examples
///
/// ```rust,ignore
/// use std::{io, path::Path};
/// let err = io::Error::new(io::ErrorKind::NotFound, "missing");
/// let msg = normalised_dir_read_error(Path::new("dir"), &err);
/// assert_eq!(msg, "failed to read directory `dir`: directory not found");
/// ```
#[must_use]
pub(crate) fn normalised_dir_read_error(path: &std::path::Path, err: &std::io::Error) -> String {
    match err.kind() {
        std::io::ErrorKind::NotFound => {
            format!(
                "failed to read directory `{}`: directory not found",
                path.display()
            )
        }
        _ => format!("failed to read directory `{}`: {err}", path.display()),
    }
}
