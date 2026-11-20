//! Where Cargo's home directory is, for the configuration walk.
//!
//! The walk reads `$CARGO_HOME/config.toml` as well as the files above the
//! working directory, so naming the same home Cargo itself would is part of
//! the fingerprint. Split from [`super`] so the resolution rule — the one place
//! a relative value is re-rooted — can be read and exercised on its own.

use std::{
    ffi::OsStr,
    path::{Path, PathBuf},
};

/// The Cargo home directory, honouring `CARGO_HOME` and Cargo's default.
///
/// An explicitly set `CARGO_HOME` wins, because that is what Cargo itself does
/// and the search must name the same directory the build reads. A relative
/// value is resolved against `working_directory`, matching Cargo's own rule
/// that it is interpreted relative to the command's working directory; the
/// nested builds run beneath the workspace, not in this process's directory, so
/// passing the value through unchanged would name two different trees.
/// Otherwise the default is the `home` directory's `.cargo`, from `HOME` on
/// Unix and `USERPROFILE` on Windows — the same pair Cargo's own default
/// resolves through. A host with neither variable set has no default to name,
/// and is left out rather than guessed at.
pub(super) fn cargo_home(working_directory: &Path) -> Option<PathBuf> {
    resolve_cargo_home(working_directory, std::env::var_os("CARGO_HOME").as_deref())
}

/// The Cargo home for an explicitly resolved `CARGO_HOME`.
///
/// Split from [`cargo_home`] so the resolution rule — the one place a relative
/// value is re-rooted — can be exercised for a chosen value, without a case
/// writing to an environment variable every other thread in this binary reads.
///
/// An empty value is treated as unset, which is what Cargo does: it then falls
/// back to `$HOME/.cargo` (or `$USERPROFILE/.cargo` on Windows).
pub(crate) fn resolve_cargo_home(
    working_directory: &Path,
    explicit: Option<&OsStr>,
) -> Option<PathBuf> {
    if let Some(explicit) = explicit {
        let explicit = PathBuf::from(explicit);
        if !explicit.as_os_str().is_empty() {
            // Cargo interprets a relative `CARGO_HOME` against the working
            // directory of the command that reads it. The nested build's
            // working directory is the workspace (or a fixture beneath it),
            // not this process's, so the value has to be joined here or the
            // probe would read one tree while the build read another. An
            // absolute value is used unchanged, as Cargo does.
            return Some(if explicit.is_absolute() {
                explicit
            } else {
                working_directory.join(explicit)
            });
        }
    }
    let home = std::env::var_os("HOME")
        .filter(|home| !home.is_empty())
        .or_else(|| std::env::var_os("USERPROFILE").filter(|home| !home.is_empty()))?;
    Some(PathBuf::from(home).join(".cargo"))
}
