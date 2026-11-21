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
/// Otherwise the default is the platform home directory's `.cargo`, resolved by
/// [`platform_home`]. A host with no home to name is left out rather than
/// guessed at.
pub(super) fn cargo_home(working_directory: &Path) -> Option<PathBuf> {
    resolve_cargo_home(
        working_directory,
        std::env::var_os("CARGO_HOME").as_deref(),
        platform_home,
    )
}

/// The Cargo home for an explicitly resolved `CARGO_HOME`.
///
/// Split from [`cargo_home`] so the resolution rule — the one place a relative
/// value is re-rooted — can be exercised for a chosen value, without a case
/// writing to an environment variable every other thread in this binary reads.
///
/// An empty value is treated as unset, which is what Cargo does: it then falls
/// back to the platform home's `.cargo` via `home`.
///
/// # Why a relative value must be re-rooted
///
/// Cargo reads a relative `CARGO_HOME` against the working directory of the
/// command that consults it. The nested build runs in the fixture or generated
/// crate, not wherever the fingerprint happens to be computed, so a value
/// passed through unchanged would point the probe at one tree and the build at
/// another. `working_directory` is therefore the *nested command's* directory
/// and not the stamped root; the two differ for the trybuild families, where
/// the generated crate lives beneath `target/`.
pub(crate) fn resolve_cargo_home(
    working_directory: &Path,
    explicit: Option<&OsStr>,
    home: fn() -> Option<PathBuf>,
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
    Some(home()?.join(".cargo"))
}

/// The platform home directory, resolved the way Cargo resolves it.
///
/// Cargo's `home` crate defines its own `home_dir` because the standard
/// library's used to consult `HOME` on Windows, which made a Rust program
/// behave differently under Cygwin or MinGW. That divergence is gone — std
/// changed in 1.85 — but Cargo's definition is still the one that decides
/// which `.cargo` a build reads, so the rule is taken from its source rather
/// than inferred from what `std::env::home_dir` happens to do.
///
/// On **Windows** the two now agree: `USERPROFILE` when set and non-empty,
/// otherwise the OS profile folder (`GetUserProfileDirectory` in std,
/// `SHGetKnownFolderPath(FOLDERID_Profile)` in Cargo — the same directory by a
/// different call). `HOME` is never consulted by either, and consulting it
/// would name a `.cargo` the build never reads whenever `HOME` and
/// `USERPROFILE` disagree.
///
/// On **Unix** Cargo delegates to `std::env::home_dir` outright, so this does
/// too. `HOME` when set — including when set to the empty string, which Cargo
/// also returns verbatim — and otherwise the passwd entry for the current uid,
/// so the `getpwuid_r` fallback comes with it. That fallback is the case that
/// matters here: without it a host with `HOME` unset would report no home at
/// all, drop the real `.cargo` out of the fingerprint, and let a configuration
/// edit leave the stamp valid.
pub(super) fn platform_home() -> Option<PathBuf> { std::env::home_dir() }

#[cfg(test)]
mod tests {
    //! The default Cargo home, exercised through the injected resolver.
    //!
    //! The platform rule itself reads process-wide environment, which these
    //! parallel cases cannot vary safely. Taking the home as a parameter is
    //! what lets the *rule* be pinned here; `platform_home` itself is a single
    //! call into the standard library, whose own per-platform rule is tested
    //! there.

    use std::{ffi::OsStr, path::PathBuf};

    use super::resolve_cargo_home;

    /// A resolver that reports no home at all.
    fn no_home() -> Option<PathBuf> { None }

    #[test]
    fn an_absent_cargo_home_falls_back_to_the_platform_home() {
        // The default is the platform home's `.cargo`. Getting this wrong is
        // silent: the walk would simply read a directory Cargo never consults,
        // so an edit to the real Cargo home would leave the fingerprint
        // unchanged and move the rebuild into a timed assertion.
        let resolved = resolve_cargo_home(&PathBuf::from("/srv/workspace"), None, || {
            Some(PathBuf::from("/home/tester"))
        });
        assert_eq!(
            resolved,
            Some(PathBuf::from("/home/tester").join(".cargo")),
            "the default must be the platform home's `.cargo`"
        );
    }

    #[test]
    fn an_empty_cargo_home_is_treated_as_unset() {
        // Cargo treats an empty value as unset — not as the working directory,
        // which is what joining an empty relative path would produce.
        let resolved = resolve_cargo_home(
            &PathBuf::from("/srv/workspace"),
            Some(OsStr::new("")),
            || Some(PathBuf::from("/home/tester")),
        );
        assert_eq!(
            resolved,
            Some(PathBuf::from("/home/tester").join(".cargo")),
            "an empty CARGO_HOME must fall through to the default, not become the workspace"
        );
    }

    #[test]
    fn a_relative_cargo_home_resolves_against_the_command_directory() {
        // The finding this fixes: a relative value must follow the nested
        // command, which is not necessarily the directory being walked.
        let command_directory = PathBuf::from("/srv/target/tests/trybuild/pkg");
        let resolved =
            resolve_cargo_home(&command_directory, Some(OsStr::new("cargo-home")), no_home);
        assert_eq!(
            resolved,
            Some(command_directory.join("cargo-home")),
            "a relative CARGO_HOME belongs to the command's directory"
        );
    }

    #[test]
    fn an_absolute_cargo_home_ignores_the_command_directory() {
        let resolved = resolve_cargo_home(
            &PathBuf::from("/srv/target/tests/trybuild/pkg"),
            Some(OsStr::new("/opt/cargo-home")),
            no_home,
        );
        assert_eq!(
            resolved,
            Some(PathBuf::from("/opt/cargo-home")),
            "an absolute CARGO_HOME must be used unchanged"
        );
    }

    #[test]
    fn no_home_anywhere_yields_none_rather_than_a_guess() {
        // A host with no home to name has no default to report. Inventing one
        // would put a directory into the fingerprint that Cargo does not read.
        assert_eq!(
            resolve_cargo_home(&PathBuf::from("/srv"), None, no_home),
            None
        );
    }
}
