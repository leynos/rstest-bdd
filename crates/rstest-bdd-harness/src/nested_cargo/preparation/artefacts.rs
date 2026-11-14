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
//! has lost the directories Cargo reads to decide its units are current. The
//! exact layer is the one that makes the check sound: the stamp records the
//! outputs the preparation left beside its dep-info records, and every one of
//! them must still be present.
//!
//! A rule about rustc's file naming cannot decide this. Removing only
//! `lib<stem>.rlib` while `lib<stem>.rmeta` survives makes Cargo recompile
//! that unit — measured against a real crate, not assumed — and the tree that
//! leaves behind is the same shape a `cargo check` leaves for a genuine
//! metadata-only unit, which Cargo does reuse. No predicate over file names
//! separates those two states. A record of what the preparation produced does,
//! because a unit that was linked has an `.rlib` in it and a check-only unit
//! never does.
//!
//! Neither layer names a directory. Cargo's layout is explicitly unstable and
//! changed under this module once already; [`super::layout`] discovers where the
//! units are, and this module records what it found there.

use std::{
    fs,
    io,
    path::{Path, PathBuf},
};

use super::{
    Preparation,
    layout::{self, Layout},
    stamp::Stamp,
};

mod replacement;

/// How many missing names a failure message lists before summarizing.
///
/// A `cargo clean` can take hundreds of entries at once. Naming every one
/// turns a one-line diagnosis into a log flood — the first run of an earlier
/// version of this check against this workspace printed 108 KB — without
/// telling the reader anything past the first few. The count is never dropped,
/// only the tail.
const NAMED_OUTPUTS: usize = 5;

/// Why a prepared profile is no longer usable.
#[derive(Debug, Clone)]
pub(super) struct Eviction {
    /// The profile directory whose artefacts are missing.
    pub(super) profile: PathBuf,
    /// What is wrong with the profile as a whole, when no single recorded
    /// output can be named.
    pub(super) reason: Option<&'static str>,
    /// The recorded artefact names that have gone.
    pub(super) missing: Vec<String>,
}

impl Eviction {
    /// An eviction of the whole profile, described by `reason`.
    fn whole(directory: &Path, reason: &'static str) -> Self {
        Self {
            profile: directory.to_path_buf(),
            reason: Some(reason),
            missing: Vec::new(),
        }
    }

    /// Describe this eviction for a failure message.
    pub(super) fn describe(&self) -> String {
        let profile = self.profile.display();
        if self.missing.is_empty() {
            return self.reason.map_or_else(
                || profile.to_string(),
                |reason| format!("{profile} ({reason})"),
            );
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
            let _ = std::fmt::Write::write_fmt(&mut listed, format_args!(" and {remaining} more"));
        }
        format!("{profile} (missing {listed})")
    }
}

/// The name a recorded output is filed under.
///
/// The unit directory is named relative to the profile and joined with `/` on
/// every platform, so the same tree produces the same stamp text on Windows and
/// on Unix, and a stamp written by one host is readable by another. The
/// directory has to be part of the name: two units in different directories can
/// share a stem, and a record that named only the stem would let one unit's
/// output witness the other's.
fn recorded_name(profile: &Path, directory: &Path, output: &str) -> String {
    let relative: Vec<String> = directory.strip_prefix(profile).map_or_else(
        |_| vec![directory.to_string_lossy().into_owned()],
        |relative| {
            relative
                .components()
                .map(|part| part.as_os_str().to_string_lossy().into_owned())
                .collect()
        },
    );
    if relative.is_empty() {
        return output.to_owned();
    }
    format!("{}/{output}", relative.join("/"))
}

/// The recorded names a layout vouches for, sorted.
///
/// # Errors
///
/// Returns the read error when a unit directory cannot be listed.
fn recorded_names(profile: &Path, layout: &Layout) -> io::Result<Vec<String>> {
    let mut names: Vec<String> = Vec::new();
    for directory in &layout.units {
        for output in layout::outputs_in(directory)? {
            names.push(recorded_name(profile, directory, &output));
        }
    }
    names.sort();
    names.dedup();
    Ok(names)
}

/// Record the artefact names a freshly prepared family vouches for.
///
/// Called after the preparation command succeeded, so the names are what that
/// command actually left rather than what it was expected to leave. One list
/// per [`Preparation::artefacts`] entry, in that order.
///
/// A profile that does not exist records nothing rather than failing: the
/// caller runs [`verify_structural`] first, which has already refused that
/// state with the diagnosis worth reporting. Reaching here with one absent
/// would mean an eviction between the two calls, and an empty record there is
/// still refused by [`verify_recorded`]'s structural half.
///
/// A name that is not valid Unicode is recorded lossily: every real unit name is
/// ASCII, and refusing on one would fail a preparation for a reason it cannot
/// name.
///
/// # Errors
///
/// Returns the read error when a profile that exists cannot be inspected.
pub(super) fn record(preparation: &Preparation) -> io::Result<Vec<Vec<String>>> {
    preparation
        .artefacts
        .iter()
        .map(|profile| recorded_names(profile, &layout::discover(profile)?))
        .collect()
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
        Ok(_) => return Ok(Some(Eviction::whole(directory, "not a directory"))),
        Err(error) if error.kind() == io::ErrorKind::NotFound => {
            return Ok(Some(Eviction::whole(directory, "absent")));
        }
        // A permission or I/O fault must name itself. Folding it into an
        // eviction would report a tree that cannot be inspected as one that is
        // known to be empty, which is a diagnosis the reader cannot act on.
        Err(error) => return Err(error),
    }
    let layout = layout::discover(directory)?;
    // Naming the directory is the whole diagnosis when a half is missing;
    // nothing inside it can be reported as individually gone. The two halves
    // are ordered so the message names the one the preparation most likely
    // lost first, and `is_usable` remains the single definition of "both".
    if layout.units.is_empty() {
        return Ok(Some(Eviction::whole(directory, "holds no compiled units")));
    }
    if layout.fingerprints.is_empty() {
        return Ok(Some(Eviction::whole(
            directory,
            "holds no fingerprint records",
        )));
    }
    debug_assert!(layout.is_usable(), "both halves were just checked");
    let Some(recorded) = recorded else {
        return Ok(None);
    };
    let present = recorded_names(directory, &layout)?;
    let missing: Vec<String> = recorded
        .iter()
        .filter(|name| !replacement::is_vouched_for(name.as_str(), &present))
        .cloned()
        .collect();
    if missing.is_empty() {
        return Ok(None);
    }
    Ok(Some(Eviction {
        profile: directory.to_path_buf(),
        reason: None,
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
