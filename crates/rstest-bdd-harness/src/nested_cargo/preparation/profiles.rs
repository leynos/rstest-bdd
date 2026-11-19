//! Which profile directories a preparation's stamp vouches for.
//!
//! Cargo writes its compiled units into a profile directory below the target
//! directory, and which directories those are depends on the target selection.
//! Without one there is a single `<target>/debug`. With `--target <triple>` (or
//! `CARGO_BUILD_TARGET`, its environment spelling) there are two:
//! `<target>/<triple>/debug` holds the units built for that triple, and
//! `<target>/debug` holds the host-side units the same build needs — its build
//! scripts and proc macros.
//!
//! Measured on this host, not assumed: a leaf crate built with
//! `--target x86_64-unknown-linux-gnu` left `libleaf.d` and `libleaf.rlib`
//! under `<target>/x86_64-unknown-linux-gnu/debug`, and only lock files,
//! `build/`, `examples/` and `incremental/` under `<target>/debug`. A crate
//! with a proc-macro dependency left its host units under
//! `<target>/debug/build/...` and its own under the triple. `cargo metadata`
//! reports its target directory as `$CARGO_TARGET_DIR` unchanged in both cases,
//! so a descriptor derived from it alone names a profile holding the wrong half
//! of the tree.

use std::{
    ffi::OsStr,
    path::{Path, PathBuf},
};

use super::{DEFAULT_PROFILE, NestedCargoEnv};

/// The profiles a nested build writes beneath `target`.
///
/// Without a target selection there is one, `<target>/debug`, and that is the
/// only directory this function names. A build selected onto a triple through
/// `CARGO_BUILD_TARGET` writes *two*: `<target>/<triple>/debug` holds the units
/// built for that triple, while `<target>/debug` holds the host-side units the
/// same build needs, which are its build scripts and proc macros. The host
/// profile is therefore not empty in that mode and is not the profile a timed
/// assertion reuses.
///
/// Naming only `<target>/debug` is what went wrong here. A descriptor derived
/// from the target directory alone vouched for a profile holding the host units
/// while the fixture's own units sat in the triple's directory, so evicting
/// that directory left the gate admitting a tree the timed assertion would
/// rebuild. Cargo applies the same selection to the directory it writes, so
/// both profiles are named whenever the variable selects one.
///
/// The triple is **not** validated or resolved against rustc's list of built-in
/// targets. Doing so would need a subprocess here, in a constructor that must
/// stay cheap and infallible apart from the target-directory lookup, and a
/// triple Cargo rejects fails the preparation command itself with a far better
/// message than this could produce. A value Cargo accepts as a path component
/// but does not treat as a target is the one case where an extra directory is
/// named; it is absent, and the gate then refuses a preparation that would
/// otherwise have been admitted — the safe direction.
///
/// A path-valued selection (`--target <path>/spec.json`) names the profile by
/// the specification file's stem, which is what Cargo does.
///
/// Only the selection this environment names is considered. A `build.target`
/// in a configuration file the child reads is invisible here, and is not
/// covered: the value would have to be discovered by reading the same files
/// `cargo_config_inputs` names, and a family relying on one would need a
/// complete profile list from somewhere other than this constructor. The
/// environment route is the one a CI job sets, and is the case reported against
/// this suite.
pub(super) fn profiles(target: &Path, environment: &NestedCargoEnv) -> Vec<PathBuf> {
    let host = target.join(DEFAULT_PROFILE);
    let Some(selection) = target_selection(environment) else {
        return vec![host];
    };
    // `<target>/<triple>/debug`, matching Cargo's own layout. A selection that
    // carries no usable last component cannot name a profile, so the host
    // profile alone is named and the preparation fails on the missing
    // directory rather than on a guessed one.
    match profile_name(&selection) {
        Some(name) => vec![host, target.join(name).join(DEFAULT_PROFILE)],
        None => vec![host],
    }
}

/// The directory name Cargo gives a selected target's profile.
///
/// A specification file is named by its stem, which is Cargo's rule for a
/// path-valued `--target`. A built-in target is named by the whole triple, and
/// the distinction cannot be made with [`Path::file_stem`]: a triple such as
/// `thumbv8m.main-none-eabi` carries a dot, so a stem would cut it to
/// `thumbv8m` and name a directory Cargo never writes. The gate would then
/// refuse every prepared family on such a host — the failure mode this
/// function's callers exist to avoid.
///
/// Only a value that ends in `.json` is treated as a specification file,
/// because that is the extension Cargo requires for one. Every other value is
/// kept in full, including a triple whose last component happens to look like
/// an extension.
///
/// A selection with no usable final component — `.`, `..`, `/` — cannot name a
/// directory, and yields `None` so the caller vouches for the host profile
/// alone rather than for an unrelated tree.
fn profile_name(selection: &Path) -> Option<&OsStr> {
    let name = selection.file_name()?;
    if name.is_empty() {
        return None;
    }
    if selection
        .extension()
        .is_some_and(|extension| extension == "json")
    {
        return non_empty(selection.file_stem());
    }
    Some(name)
}

/// `candidate`, or `None` when it is empty.
///
/// A specification file named `.json` has an empty stem, which names no
/// directory.
fn non_empty(candidate: Option<&OsStr>) -> Option<&OsStr> {
    candidate.filter(|value| !value.is_empty())
}

/// The target selection the nested build's environment names, if any.
///
/// `CARGO_BUILD_TARGET` is Cargo's environment spelling for the `build.target`
/// setting, and `stamp` already treats the whole `CARGO_BUILD_` prefix
/// as build-relevant, so this is the same value the fingerprint changes on.
///
/// The value is trimmed before it is used, matching Cargo, which accepts a
/// padded `build.target` and selects the unpadded triple. Left untrimmed, a
/// selection of `" x86_64-unknown-linux-gnu "` would name a profile
/// `target/" x86_64-unknown-linux-gnu "/debug` that Cargo never writes, and the
/// gate would refuse a tree that is prepared.
fn target_selection(environment: &NestedCargoEnv) -> Option<PathBuf> {
    let value = environment.get("CARGO_BUILD_TARGET")?;
    // Cargo parses `build.target` as a string, so a value that is not valid
    // UTF-8 is one Cargo cannot use either; read it the same way rather than
    // failing the descriptor for a value the build would reject first.
    let trimmed = value.to_string_lossy();
    let trimmed = trimmed.trim();
    if trimmed.is_empty() {
        return None;
    }
    Some(PathBuf::from(trimmed))
}

#[cfg(test)]
mod tests;
