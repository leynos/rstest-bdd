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

/// Whether `name` is an output that belongs to the unit with `stem`.
///
/// This is deliberately a *rule about rustc's naming*, not a list of
/// extensions. An earlier version enumerated the forms it expected and was
/// wrong twice in a row, each time refusing a genuine tree in CI: first it
/// accepted only `lib<stem>.rlib` and refused metadata-only units, then it
/// listed `so` and `dylib` and refused Windows proc-macro units, which are
/// `lib<stem>.dll`. The extensions differ per target and per crate type —
/// `rlib`, `rmeta`, `so`, `dylib`, `dll`, `a`, `exe`, none — so any list is a
/// guess about a toolchain this code does not control.
///
/// The rule instead uses what *is* stable: rustc names a library output
/// `lib<stem>.<something>` and a linked binary `<stem>` or `<stem>.<something>`
/// (`.exe` on Windows). Anchoring on the stem and requiring a separator keeps
/// the pairing from matching a different unit whose name merely starts with
/// the same characters.
fn is_output_of(name: &str, stem: &str) -> bool {
    // A library: `lib<stem>` followed by an extension.
    let library = name
        .strip_prefix("lib")
        .and_then(|rest| rest.strip_prefix(stem))
        .is_some_and(|extension| extension.starts_with('.'));
    // A linked binary: exactly `<stem>`, or `<stem>.<extension>`.
    let binary = name
        .strip_prefix(stem)
        .is_some_and(|rest| rest.is_empty() || rest.starts_with('.'));
    library || binary
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
    let present: Vec<String> = entries
        .filter_map(Result::ok)
        .filter_map(|entry| entry.file_name().to_str().map(str::to_owned))
        .collect();
    // A record's own name must not count as its output: `unit.d` matches
    // `is_output_of(_, "unit")` because `.d` is a dot-extension, so pairing a
    // record against the whole listing would make every record its own
    // witness and the check would always pass.
    let mut orphaned: Vec<String> = present
        .iter()
        .filter_map(|name| dep_info_stem(name).map(|stem| (name, stem)))
        .filter(|(record, stem)| {
            !present
                .iter()
                .any(|name| name != *record && is_output_of(name, stem))
        })
        .map(|(_, stem)| stem)
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

#[cfg(test)]
mod tests {
    //! Unit tests for the dep-info pairing rule and its listing scan.

    use super::{is_output_of, orphaned_dep_info};

    /// Every output form rustc has been observed to leave pairs with its
    /// record.
    ///
    /// The list is a record of what CI and this host actually produced, not a
    /// claim about every target: the predicate is a rule, so it does not need
    /// to enumerate them. Both entries that broke earlier versions are here —
    /// the metadata-only unit and the Windows proc-macro `.dll` — because
    /// those are the failures a list-based predicate produced.
    #[test]
    fn observed_output_forms_pair_with_their_record() {
        for output in [
            "libunit.rlib",
            "libunit.rmeta",
            "libunit.so",
            "libunit.dylib",
            "libunit.dll",
            "libunit.a",
            "unit",
            "unit.exe",
        ] {
            assert!(
                is_output_of(output, "unit"),
                "{output} is an output of unit and must pair with unit.d"
            );
        }
    }

    /// A different unit whose name merely begins with the stem does not pair.
    ///
    /// Without the separator check, `unit_extra.rlib` would be read as
    /// `unit`'s output, so removing the real `unit` output would go unnoticed
    /// whenever a similarly named unit happened to be present.
    #[test]
    fn a_longer_stem_is_not_an_output() {
        for unrelated in [
            "libunit_extra.rlib",
            "libunit_extra.rmeta",
            "unit-2",
            "unit_extra",
            "unit_extra.exe",
            "libunits.rlib",
        ] {
            assert!(
                !is_output_of(unrelated, "unit"),
                "{unrelated} belongs to another unit and must not pair with unit.d"
            );
        }
    }

    /// Only records are considered, and each record must be backed by a name
    /// other than itself.
    #[test]
    fn a_record_is_not_its_own_output() {
        let directory = tempfile::tempdir().expect("temp dir");
        std::fs::write(directory.path().join("unit.d"), b"dep-info").expect("record");

        assert_eq!(
            orphaned_dep_info(directory.path()),
            vec!["unit".to_owned()],
            "a lone record describes nothing"
        );

        std::fs::write(directory.path().join("libunit.rmeta"), b"meta").expect("output");
        assert!(
            orphaned_dep_info(directory.path()).is_empty(),
            "a metadata-only unit must pair"
        );
    }
}
