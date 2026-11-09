//! What a prepared profile must still contain for a stamp to be believed.
//!
//! A stamp answers "the inputs and environment have not changed since this
//! family was prepared". It cannot answer "the compiled units are still
//! there": the profile directory holding them need not be inside the family's
//! stamp root, so `cargo clean`, a `rm -rf <target>/debug`, or any tool that
//! evicts only the profile can leave the stamp untouched. This module supplies
//! the second half of the answer, in two layers.
//!
//! The structural layer refuses a profile that exists and is empty, or that
//! has lost the subdirectories Cargo reads to decide its units are current.
//! The exact layer is the one that makes the check sound: the stamp records
//! the outputs the preparation left beside its dep-info records, and every one
//! of them must still be present.
//!
//! A rule about rustc's file naming cannot decide this. Removing only
//! `lib<stem>.rlib` while `lib<stem>.rmeta` survives makes Cargo recompile
//! that unit — measured against a real crate, not assumed — and the tree that
//! leaves behind is the same shape a `cargo check` leaves for a genuine
//! metadata-only unit, which Cargo does reuse. No predicate over file names
//! separates those two states. A record of what the preparation produced does,
//! because a unit that was linked has an `.rlib` in it and a check-only unit
//! never does.

use std::{
    collections::BTreeSet,
    fmt::Write as _,
    fs,
    io,
    path::{Path, PathBuf},
};

use super::{Preparation, stamp::Stamp};

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
///
/// An absent directory is reported as "no entries" rather than as an error:
/// that is the ordinary eviction this module exists to name, and surfacing it
/// as a bare `No such file or directory` would replace a precise diagnosis
/// with a filesystem message.
///
/// # Errors
///
/// Returns the read error for any failure other than absence — a permission
/// denial, an I/O fault. Those must fail closed rather than be read as an
/// empty directory, which is the unsafe direction: it would let an unreadable
/// profile pass the check.
fn has_entries(directory: &Path) -> io::Result<bool> {
    match fs::read_dir(directory) {
        Ok(mut entries) => has_entries_from(entries.next()),
        Err(error) if error.kind() == io::ErrorKind::NotFound => Ok(false),
        Err(error) => Err(error),
    }
}

/// Decide whether a listing is non-empty from its first step.
///
/// `read_dir` yields each entry lazily, so its error arrives from `next()`. An
/// `Err` there is a read fault partway through the listing, not evidence of an
/// entry: counting it as one would report an unreadable directory as populated,
/// the same fail-open direction [`has_entries`] guards. No portable fixture
/// produces that error as the *first* item, hence this seam.
fn has_entries_from(first: Option<io::Result<fs::DirEntry>>) -> io::Result<bool> {
    match first {
        Some(Ok(_)) => Ok(true),
        Some(Err(error)) => Err(error),
        None => Ok(false),
    }
}

/// The entry names in `directory`, sorted.
///
/// Sorting keeps the recorded list — which becomes part of the stamp — stable
/// across runs. `read_dir` order is the filesystem's, so an unsorted list would
/// rewrite the stamp on every preparation on some hosts.
///
/// # Errors
///
/// Returns the read error when any entry cannot be named.
fn entries_of(directory: &Path) -> io::Result<Vec<String>> {
    let mut names: Vec<String> = fs::read_dir(directory)?
        .map(|entry| entry.map(|entry| entry.file_name().to_string_lossy().into_owned()))
        .collect::<io::Result<_>>()?;
    names.sort();
    Ok(names)
}

/// Split a dep-info file name into its unit stem.
///
/// Returns `None` for anything that is not a `.d` record, so an unrelated
/// entry in `deps` — a `.rlib`, a `.rmeta`, a build script's output — is never
/// mistaken for one.
fn dep_info_stem(name: &str) -> Option<&str> { name.strip_suffix(".d") }

/// The outputs in `present` that belong to the unit with `stem`.
///
/// This is a *rule about rustc's naming*, not a list of extensions, and it is
/// used only to discover which entries belong together — never to decide that
/// a tree is usable. An earlier version used it for the decision itself and
/// was wrong: see the module documentation.
///
/// The rule uses what is stable: rustc names a library output
/// `lib<stem>.<something>` and a linked binary `<stem>` or `<stem>.<something>`
/// (`.exe` on Windows). Anchoring on the stem and requiring a separator keeps
/// the pairing from matching a different unit whose name merely starts with
/// the same characters.
fn is_output_of(name: &str, stem: &str) -> bool {
    // A record's own name must not count as its output: `unit.d` matches the
    // binary form because `.d` is a dot-extension, so a unit with no output
    // would otherwise appear to witness itself.
    if name == format!("{stem}.d") {
        return false;
    }
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

/// The outputs a profile's dep-info records describe.
///
/// These are the units Cargo will recompile if their outputs go missing, so
/// they are exactly the names a preparation must be able to vouch for. The
/// list is one directory listing, reused for every record: `deps` holds
/// thousands of entries and this runs once per preparation.
///
/// # Errors
///
/// Returns the read error when `deps` cannot be listed.
fn described_outputs(deps: &Path) -> io::Result<Vec<String>> {
    let present = entries_of(deps)?;
    let mut outputs: BTreeSet<String> = BTreeSet::new();
    for stem in present.iter().filter_map(|name| dep_info_stem(name)) {
        outputs.extend(
            present
                .iter()
                .filter(|name| is_output_of(name, stem))
                .cloned(),
        );
    }
    Ok(outputs.into_iter().collect())
}

/// Record the artefact names a freshly prepared family vouches for.
///
/// Called after the preparation command succeeded, so the names are what that
/// command actually left rather than what it was expected to leave. One list
/// per [`Preparation::artefacts`] entry, in that order.
///
/// A `deps` directory that does not exist records nothing rather than
/// failing: the caller runs [`verify_structural`] first, which has already
/// refused that state with the diagnosis worth reporting. Reaching here with
/// one absent would mean an eviction between the two calls, and an empty
/// record there is still refused by [`verify_recorded`]'s structural half.
///
/// A name that is not valid Unicode is recorded lossily: every real `deps`
/// entry is ASCII, and refusing on one would fail a preparation for a reason
/// it cannot name.
///
/// # Errors
///
/// Returns the read error when a `deps` directory that exists cannot be
/// listed, or an entry cannot be named.
pub(super) fn record(preparation: &Preparation) -> io::Result<Vec<Vec<String>>> {
    preparation
        .artefacts
        .iter()
        .map(
            |directory| match described_outputs(&directory.join("deps")) {
                Ok(outputs) => Ok(outputs),
                Err(error) if error.kind() == io::ErrorKind::NotFound => Ok(Vec::new()),
                Err(error) => Err(error),
            },
        )
        .collect()
}

/// Why one prepared profile is no longer usable.
#[derive(Debug, Clone)]
pub(super) struct Eviction {
    /// The profile directory whose artefacts are missing.
    pub(super) profile: PathBuf,
    /// The recorded artefact names that have gone.
    pub(super) missing: Vec<String>,
}

/// How many missing names a failure message lists before summarizing.
///
/// A `cargo clean` can take hundreds of entries at once. Naming every one
/// turns a one-line diagnosis into a log flood — the first run of an earlier
/// version of this check against this workspace printed 108 KB — without
/// telling the reader anything past the first few. The count is never dropped,
/// only the tail.
const NAMED_OUTPUTS: usize = 5;

impl Eviction {
    /// An eviction of the whole profile, naming no individual output.
    fn whole(directory: &Path) -> Self {
        Self {
            profile: directory.to_path_buf(),
            missing: Vec::new(),
        }
    }

    /// Describe this eviction for a failure message.
    pub(super) fn describe(&self) -> String {
        let profile = self.profile.display();
        if self.missing.is_empty() {
            return profile.to_string();
        }
        let named: Vec<&str> = self
            .missing
            .iter()
            .take(NAMED_OUTPUTS)
            .map(String::as_str)
            .collect();
        let mut listed = named.join(", ");
        let remaining = self.missing.len().saturating_sub(named.len());
        if remaining > 0 {
            // Writing into the existing buffer avoids a second allocation for
            // a suffix that is at most a few characters, and is what clippy's
            // `format_push_string` asks for.
            let _ = write!(listed, " and {remaining} more");
        }
        format!("{profile} (missing {listed})")
    }
}

/// Why `directory` is not a usable prepared profile, if it is not.
///
/// # Errors
///
/// Returns the read error when a directory cannot be inspected, so an
/// unreadable tree refuses the preparation instead of passing it.
fn eviction_of(directory: &Path, recorded: Option<&[String]>) -> io::Result<Option<Eviction>> {
    match fs::metadata(directory) {
        Ok(metadata) if metadata.is_dir() => {}
        // Absent, or present as something that is not a directory: both are the
        // ordinary eviction named above.
        Ok(_) => return Ok(Some(Eviction::whole(directory))),
        Err(error) if error.kind() == io::ErrorKind::NotFound => {
            return Ok(Some(Eviction::whole(directory)));
        }
        // A permission or I/O fault must name itself. Folding it into an
        // eviction would report a tree that cannot be inspected as one that is
        // known to be empty, which is a diagnosis the reader cannot act on.
        Err(error) => return Err(error),
    }
    let deps = directory.join("deps");
    if !has_entries(&deps)? {
        // Naming the directory is the whole diagnosis when it is empty or
        // absent; nothing inside it can be reported as individually gone.
        return Ok(Some(Eviction::whole(directory)));
    }
    if !has_entries(&directory.join(".fingerprint"))? {
        return Ok(Some(Eviction::whole(directory)));
    }
    let Some(recorded) = recorded else {
        return Ok(None);
    };
    let present = entries_of(&deps)?;
    let missing: Vec<String> = recorded
        .iter()
        .filter(|name| !present.contains(name))
        .cloned()
        .collect();
    if missing.is_empty() {
        return Ok(None);
    }
    Ok(Some(Eviction {
        profile: directory.to_path_buf(),
        missing,
    }))
}

/// The artefact directories the stamp vouches for that are no longer usable.
///
/// Shared by the gate and its failure message, so "the gate refused" and
/// "the message names why" can never disagree.
///
/// `stamp` is the matching stamp, when one was found. Without it there is
/// nothing below the directories to check, and the structural layer alone
/// decides.
///
/// # Errors
///
/// Returns the read error when any directory cannot be inspected.
pub(super) fn evicted(
    preparation: &Preparation,
    stamp: Option<&Stamp>,
) -> io::Result<Vec<Eviction>> {
    preparation
        .artefacts
        .iter()
        .enumerate()
        .map(|(index, directory)| {
            let recorded = stamp.and_then(|stamp| stamp.recorded.get(index));
            eviction_of(directory, recorded.map(Vec::as_slice))
        })
        .filter_map(Result::transpose)
        .collect()
}

/// Whether every directory the stamp vouches for still holds its artefacts.
///
/// An empty [`Preparation::artefacts`] is treated as satisfied: a caller that
/// described no artefact directories has nothing for this check to refute, and
/// its stamp remains the whole gate.
///
/// # Errors
///
/// Returns the read error when any directory cannot be inspected.
pub(super) fn all_present(preparation: &Preparation, stamp: Option<&Stamp>) -> io::Result<bool> {
    Ok(evicted(preparation, stamp)?.is_empty())
}

/// Refuse a just-finished preparation whose profile is not structurally usable.
///
/// A preparation command can succeed — Cargo exits zero — while the artefacts
/// it was meant to produce are not there, for example when a concurrent `cargo
/// clean` ran between the build and this check. Writing a stamp over that
/// state would advertise a prepared family whose next timed assertion pays for
/// the rebuild, so the caller is told which directories are missing instead.
///
/// # Errors
///
/// Returns [`io::ErrorKind::NotFound`] naming every unusable profile, and the
/// read error when a profile cannot be inspected.
pub(super) fn verify_structural(preparation: &Preparation) -> io::Result<()> {
    refuse_evictions(preparation, &evicted(preparation, None)?)
}

/// Refuse a just-finished preparation whose recorded artefacts have gone.
///
/// The second half of [`verify_structural`]: a unit's outputs can disappear
/// between being recorded and being stamped, which leaves every directory the
/// structural layer names in place.
///
/// # Errors
///
/// As [`verify_structural`].
pub(super) fn verify_recorded(
    preparation: &Preparation,
    recorded: &[Vec<String>],
) -> io::Result<()> {
    let stamp = Stamp {
        recorded: recorded.to_vec(),
    };
    refuse_evictions(preparation, &evicted(preparation, Some(&stamp))?)
}

/// Turn a non-empty eviction list into the preparation failure.
fn refuse_evictions(preparation: &Preparation, evicted: &[Eviction]) -> io::Result<()> {
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
mod tests;
