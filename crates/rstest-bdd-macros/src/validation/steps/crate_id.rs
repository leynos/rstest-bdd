use std::sync::LazyLock;

use camino::{Utf8Path, Utf8PathBuf};
use cap_std::{ambient_authority, fs_utf8::Dir};

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

pub(super) fn canonicalise_out_dir(original: &Utf8Path) -> Utf8PathBuf {
    canonicalise_with_cap_std(original).unwrap_or_else(|_| {
        original
            .canonicalize_utf8()
            .unwrap_or_else(|_| original.to_owned())
    })
}

pub(super) fn canonicalise_with_cap_std(
    original: &Utf8Path,
) -> Result<Utf8PathBuf, std::io::Error> {
    let dir = Dir::open_ambient_dir(".", ambient_authority())?;
    let candidate = dir.canonicalize(original)?;
    Ok(ensure_absolute(candidate, original))
}

pub(super) fn ensure_absolute(candidate: Utf8PathBuf, original: &Utf8Path) -> Utf8PathBuf {
    if candidate.is_absolute() {
        return candidate;
    }

    absolutise_relative(&candidate)
        .or_else(|| original.canonicalize_utf8().ok())
        .unwrap_or_else(|| original.to_owned())
}

pub(super) fn absolutise_relative(candidate: &Utf8Path) -> Option<Utf8PathBuf> {
    let cwd = std::env::current_dir().ok()?;
    let cwd = Utf8PathBuf::from_path_buf(cwd).ok()?;
    let joined = cwd.join(candidate);
    Some(joined.as_path().canonicalize_utf8().unwrap_or(joined))
}
