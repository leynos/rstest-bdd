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
//! subdirectories Cargo reads to decide its units are current, nor one whose
//! `deps` still holds records for outputs that have gone.

use std::{
    collections::HashSet,
    fmt::Write as _,
    fs,
    io,
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

/// The outputs rustc may leave for a unit, keyed by the unit's stem.
///
/// Every name rustc writes for a unit whose dep-info record is
/// `<stem>.d`, so a record is "paired" when any of them exists. The set must
/// hold all of them: a unit compiled for `cargo check` or as a dependency
/// leaves only `lib<stem>.rmeta`, and proc-macro and `cdylib` units leave a
/// shared object. Accepting a narrower set refuses real trees — measured
/// against this workspace's own `target/debug/deps`, which holds 1,295
/// dep-info records against 739 `rlib`, 1,061 `rmeta` and 83 `so` outputs.
///
/// The binary forms are `<stem>` and, on Windows, `<stem>.exe`. They are
/// checked last because they are the only ones that can collide with an
/// unrelated file name.
fn candidate_outputs(stem: &str) -> [String; 6] {
    [
        format!("lib{stem}.rmeta"),
        format!("lib{stem}.rlib"),
        format!("lib{stem}.so"),
        format!("lib{stem}.dylib"),
        stem.to_owned(),
        format!("{stem}.exe"),
    ]
}

/// Split a dep-info file name into its unit stem.
///
/// Returns `None` for anything that is not a `.d` record, so an unrelated
/// entry in `deps` — a `.rlib`, a `.rmeta`, a build script's output — is never
/// mistaken for one.
fn dep_info_stem(name: &str) -> Option<String> { name.strip_suffix(".d").map(str::to_owned) }

/// Collect the dep-info records in `deps` that no longer describe an output.
///
/// One directory listing is reused for every record rather than probing the
/// filesystem per candidate name: `deps` holds thousands of entries and this
/// runs on every gate check.
///
/// A name that is not valid Unicode is skipped rather than judged: it is not a
/// Rust unit's output, and refusing on it would fail the gate for a reason it
/// cannot name. Every real `deps` entry is ASCII.
fn orphaned_dep_info(deps: &Path) -> Vec<String> {
    let Ok(entries) = fs::read_dir(deps) else {
        return Vec::new();
    };
    let present: HashSet<std::ffi::OsString> = entries
        .filter_map(Result::ok)
        .map(|entry| entry.file_name())
        .collect();
    let mut orphaned: Vec<String> = present
        .iter()
        .filter_map(|name| name.to_str())
        .filter_map(dep_info_stem)
        .filter(|stem| {
            !candidate_outputs(stem)
                .iter()
                .any(|candidate| present.contains(std::ffi::OsStr::new(candidate)))
        })
        .collect();
    // `read_dir` order is the filesystem's, not a stable sequence, and the
    // message names these stems: sorting keeps it reproducible.
    orphaned.sort();
    orphaned
}

/// Why one prepared profile is no longer usable.
#[derive(Debug, Clone)]
pub(super) struct Eviction {
    /// The profile directory whose artefacts are missing.
    pub(super) profile: PathBuf,
    /// Unit stems whose dep-info record outlived its output.
    pub(super) orphaned_outputs: Vec<String>,
}

/// How many orphaned stems a failure message names before summarizing.
///
/// A `cargo clean` can orphan hundreds of records at once. Naming every one
/// turns a one-line diagnosis into a log flood — the first run of this check
/// against this workspace printed 108 KB — without telling the reader anything
/// past the first few. The count is never dropped, only the tail.
const NAMED_OUTPUTS: usize = 5;

impl Eviction {
    /// Describe this eviction for a failure message.
    pub(super) fn describe(&self) -> String {
        let profile = self.profile.display();
        if self.orphaned_outputs.is_empty() {
            return profile.to_string();
        }
        let named: Vec<&str> = self
            .orphaned_outputs
            .iter()
            .take(NAMED_OUTPUTS)
            .map(String::as_str)
            .collect();
        let mut listed = named.join(", ");
        let remaining = self.orphaned_outputs.len().saturating_sub(named.len());
        if remaining > 0 {
            // Writing into the existing buffer avoids a second allocation for
            // a suffix that is at most a few characters, and is what clippy's
            // `format_push_string` asks for.
            let _ = write!(listed, " and {remaining} more");
        }
        format!("{profile} (missing outputs for {listed})")
    }
}

/// Why `directory` is not a usable prepared profile, if it is not.
fn eviction_of(directory: &Path) -> Option<Eviction> {
    let populated = directory.is_dir()
        && DEPENDENCY_ARTEFACT_DIRS
            .iter()
            .all(|name| has_entries(&directory.join(name)));
    let orphaned_outputs = if populated {
        orphaned_dep_info(&directory.join("deps"))
    } else {
        Vec::new()
    };
    if populated && orphaned_outputs.is_empty() {
        return None;
    }
    Some(Eviction {
        profile: directory.to_path_buf(),
        orphaned_outputs,
    })
}

/// The artefact directories the stamp vouches for that are no longer usable.
///
/// Shared by the gate and its failure message, so "the gate refused" and "the
/// message names why" can never disagree.
pub(super) fn evicted(preparation: &Preparation) -> Vec<Eviction> {
    preparation
        .artefacts
        .iter()
        .filter_map(|directory| eviction_of(directory))
        .collect()
}

/// Whether every directory the stamp vouches for still holds its artefacts.
///
/// An empty [`Preparation::artefacts`] is treated as satisfied: a caller that
/// described no artefact directories has nothing for this check to refute, and
/// its stamp remains the whole gate.
pub(super) fn all_present(preparation: &Preparation) -> bool { evicted(preparation).is_empty() }

/// Refuse a preparation that left the profile it claims to have built unusable.
///
/// A preparation command can succeed — Cargo exits zero — while the artefacts
/// it was meant to produce are not there, for example when a concurrent `cargo
/// clean` ran between the build and this check. Writing a stamp over that
/// state would advertise a prepared family whose next timed assertion pays for
/// the rebuild, so the caller is told which paths are missing instead.
pub(super) fn verify_prepared(preparation: &Preparation) -> io::Result<()> {
    let evicted = evicted(preparation);
    if evicted.is_empty() {
        return Ok(());
    }
    let described: Vec<String> = evicted.iter().map(Eviction::describe).collect();
    Err(io::Error::new(
        io::ErrorKind::NotFound,
        format!(
            "preparation of family `{}` finished but left no usable artefacts in {}",
            preparation.family,
            described.join(", ")
        ),
    ))
}
