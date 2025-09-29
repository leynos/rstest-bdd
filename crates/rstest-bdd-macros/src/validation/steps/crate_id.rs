//! Crate ID normalisation utilities for step validation.

use std::{fs, sync::LazyLock};

use camino::{Utf8Path, Utf8PathBuf};

pub(super) static CURRENT_CRATE_ID: LazyLock<Box<str>> =
    LazyLock::new(|| normalise_crate_id(&current_crate_id_raw()));

pub(super) fn current_crate_id() -> Box<str> {
    CURRENT_CRATE_ID.clone()
}

pub(super) fn normalise_crate_id(id: &str) -> Box<str> {
    let (name, path) = id.split_once(':').unwrap_or((id, ""));
    if path.is_empty() {
        return name.into();
    }

    let original = Utf8Path::new(path);
    let canonical = canonicalise_out_dir(original);
    format!("{name}:{canonical}").into_boxed_str()
}

fn current_crate_id_raw() -> String {
    // FIXME: ambient env access is read-only here; do not introduce writes (see repo guidelines).
    let name = std::env::var("CARGO_CRATE_NAME")
        .or_else(|_| std::env::var("CARGO_PKG_NAME"))
        .unwrap_or_else(|_| "unknown".to_owned());
    let out_dir = std::env::var("OUT_DIR").unwrap_or_default();
    format!("{name}:{out_dir}")
}

pub(super) fn canonicalise_out_dir(path: &Utf8Path) -> Utf8PathBuf {
    fs::canonicalize(path)
        .ok()
        .and_then(|pb| Utf8PathBuf::from_path_buf(pb).ok())
        .unwrap_or_else(|| path.to_owned())
}
