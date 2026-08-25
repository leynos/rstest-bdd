//! Crate ID normalization utilities for step validation.

use std::sync::LazyLock;

use camino::{Utf8Path, Utf8PathBuf};

/// Lazily computed normalized identifier for the current crate.
pub(super) static CURRENT_CRATE_ID: LazyLock<Box<str>> =
    LazyLock::new(|| normalize_crate_id(&current_crate_id_raw()));

/// Returns the normalized identifier for the current crate.
pub(super) fn current_crate_id() -> &'static str { CURRENT_CRATE_ID.as_ref() }

/// Normalizes a crate identifier and optional output-directory suffix.
pub(super) fn normalize_crate_id(id: &str) -> Box<str> {
    let (name, path) = id.split_once(':').unwrap_or((id, ""));
    if path.is_empty() {
        return name.into();
    }

    let original = Utf8Path::new(path);
    let canonical = canonicalize_out_dir(original);
    format!("{name}:{canonical}").into_boxed_str()
}

/// Reads the raw crate identifier from Cargo's environment.
fn current_crate_id_raw() -> String {
    // FIXME: ambient env access is read-only here; do not introduce writes (see repo guidelines).
    let name = std::env::var("CARGO_CRATE_NAME")
        .or_else(|_| std::env::var("CARGO_PKG_NAME"))
        .unwrap_or_else(|_| "unknown".to_owned());
    let out_dir = std::env::var("OUT_DIR").unwrap_or_default();
    format!("{name}:{out_dir}")
}

/// Canonicalizes an output directory when possible while preserving failures.
pub(super) fn canonicalize_out_dir(path: &Utf8Path) -> Utf8PathBuf {
    std::fs::canonicalize(path.as_std_path())
        .ok()
        .and_then(|pb| Utf8PathBuf::from_path_buf(pb).ok())
        .unwrap_or_else(|| path.to_owned())
}
