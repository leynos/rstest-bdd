//! What a prepared profile must still contain for a stamp to be believed.
//!
//! A stamp answers "the inputs and environment have not changed since this
//! family was prepared". It cannot answer "the compiled units are still
//! there": the profile directory holding them need not be inside the family's
//! stamp root, so `cargo clean`, a `rm -rf <target>/debug`, or any tool that
//! evicts only the profile can leave the stamp untouched. This module supplies
//! the second half of the answer.
//!
//! The check looks past the profile directory itself. A profile that exists
//! and is empty is not a prepared tree, and neither is one that has lost the
//! subdirectories Cargo reads to decide its units are current.

use std::{
    fs,
    path::{Path, PathBuf},
};

use super::Preparation;

/// The subdirectories Cargo needs inside a profile to skip recompiling.
///
/// `deps` holds the compiled dependency units a timed assertion links against;
/// `.fingerprint` holds the records Cargo reads to decide those units are still
/// current. Erasing either one forces the next `cargo` invocation to rebuild
/// the dependency graph, even though `<profile>` itself still exists — so a
/// gate that only checked the profile would admit exactly the cold build this
/// module exists to keep out of a timed test.
///
/// Both names are Cargo's, not this repository's: they are what
/// `<target>/<profile>` contains after any successful build, in every Cargo
/// version this workspace supports.
pub const DEPENDENCY_ARTEFACT_DIRS: [&str; 2] = ["deps", ".fingerprint"];

/// Whether `directory` exists and holds at least one entry.
///
/// An empty `deps` or `.fingerprint` is not a prepared tree: `cargo clean -p`
/// and a glob that misses both leave the directories behind and remove what
/// matters.
fn has_entries(directory: &Path) -> bool {
    fs::read_dir(directory).is_ok_and(|mut entries| entries.next().is_some())
}

/// Whether one prepared profile still holds the dependency artefacts.
fn profile_is_populated(profile: &Path) -> bool {
    profile.is_dir()
        && DEPENDENCY_ARTEFACT_DIRS
            .iter()
            .all(|name| has_entries(&profile.join(name)))
}

/// The artefact directories the stamp vouches for that are no longer usable.
///
/// Shared by the gate and its failure message, so "the gate refused" and "the
/// message names why" can never disagree.
pub(super) fn evicted(preparation: &Preparation) -> Vec<&Path> {
    preparation
        .artefacts
        .iter()
        .filter(|directory| !profile_is_populated(directory))
        .map(PathBuf::as_path)
        .collect()
}

/// Whether every directory the stamp vouches for still holds its artefacts.
///
/// An empty [`Preparation::artefacts`] is treated as satisfied: a caller that
/// described no artefact directories has nothing for this check to refute, and
/// its stamp remains the whole gate.
pub(super) fn all_present(preparation: &Preparation) -> bool { evicted(preparation).is_empty() }
