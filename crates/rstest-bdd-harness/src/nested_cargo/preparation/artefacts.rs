//! What a prepared profile must still contain for a stamp to be believed.
//!
//! A stamp answers "the inputs and environment have not changed since this
//! family was prepared". It cannot answer "the compiled units are still
//! there": the profile directory holding them need not be inside the family's
//! stamp root, so `cargo clean`, a `rm -rf <target>/debug`, or any tool that
//! evicts only the profile can leave the stamp untouched. This module supplies
//! the second half of the answer, in two layers.
//!
//! The structural layer refuses a profile that has lost the directories Cargo
//! reads to decide its units are current. It does not refuse an individual
//! profile for holding *nothing*: a build selected onto a target triple
//! legitimately leaves the host profile with only lock files, `build/` and
//! `incremental/` when nothing in the graph needs a host unit, and refusing
//! that refused every dependency-free crate built with a target selection. Such
//! a profile is excused only while another named profile holds a usable tree.
//!
//! The exact layer is the one that makes the check sound: the stamp records the
//! outputs the preparation left beside its dep-info records, and every one of
//! them must still be present. It also decides the empty-profile case on its
//! own terms — a stamp section naming artefacts that are all gone is an
//! eviction, while a section naming nothing is a profile that was empty when it
//! was prepared.
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
    io,
    path::{Path, PathBuf},
};

use state::State;

use super::{Preparation, stamp::Stamp};

mod ownership;
mod recording;
mod replacement;
mod state;
mod verification;

pub(super) use recording::record;
pub(super) use verification::{verify_declared, verify_recorded, verify_structural};

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

/// One named profile, with everything the gate needs to judge it.
///
/// The four fields travel together because they describe one directory: where
/// it is, what was found there, what the stamp claims about it, and whether an
/// empty tree may be excused at all. Passing them separately invited exactly
/// the mix-up the gate exists to prevent — a state read from one directory
/// judged against another's stamp section — and made the call site a list of
/// positional arguments whose order meant nothing.
struct ProfileUnderJudgement<'a> {
    /// The directory itself.
    directory: &'a Path,
    /// What the inspection found there.
    state: &'a State,
    /// The stamp section describing this directory, when one names it.
    recorded: Option<&'a [String]>,
    /// Whether an empty tree may be excused because a sibling holds one.
    waive_empty: bool,
}

/// Why `directory` is not a usable prepared profile, if it is not.
///
/// `recorded` carries the stamp section that describes this directory, with
/// `None` when the stamp says nothing about it. A caller with no stamp at all
/// passes `None` too, and the two are told apart by [`State::Unvouched`]: an
/// unvouched directory is refused for that reason alone, while a caller with no
/// stamp is making the structural-only answer and has no section to miss.
///
/// A profile holding *nothing* is refused here rather than in
/// [`State::unusable_reason`], because that is what tells a target-selected
/// build's empty secondary profile — which the structural layer must accept —
/// from the same profile after an eviction. The stamp records what the
/// preparation left there, so a section naming nothing is a profile that was
/// legitimately empty and stays admissible; a section naming artefacts that are
/// all gone is an eviction, and an empty presence list makes every recorded
/// name missing without a second rule.
///
/// # Errors
///
/// Returns the read error when a directory cannot be inspected, so an
/// unreadable tree refuses the preparation instead of passing it.
fn eviction_of(
    profile: &ProfileUnderJudgement<'_>,
    owned: &[String],
) -> io::Result<Option<Eviction>> {
    let ProfileUnderJudgement {
        directory,
        state,
        recorded,
        waive_empty,
    } = *profile;
    if let Some(reason) = state.absence_reason() {
        return Ok(Some(Eviction::whole(directory, reason)));
    }
    let Some(layout) = state.layout() else {
        // Absence refused both states without a layout above, so this is
        // unreachable rather than a case with an answer of its own.
        return Ok(None);
    };
    if let Some(recorded) = recorded {
        // The exact layer answers a profile holding nothing at all: a
        // target-selected build leaves its secondary profile empty, so the
        // stamp's section decides. A section naming nothing is a profile that
        // was legitimately empty and stays admissible; a section naming
        // artefacts that are all gone is an eviction, and the empty presence
        // list makes every recorded name missing without a second rule.
        let present = recording::recorded_names(directory, layout, owned)?;
        let missing = replacement::missing_names(recorded, &present);
        if missing.is_empty() {
            // The exact comparison refuses only what the section names, so a
            // section naming nothing refuses nothing however damaged the
            // directory is. That is the partial layout: compiled units still
            // present, the fingerprint store gone. A stamp recorded while the
            // profile was legitimately empty, followed by an eviction of the
            // store, would then be waived and a timed assertion would reuse a
            // tree Cargo cannot treat as current.
            //
            // The structural answer decides instead, under exactly the rule
            // the no-section path uses: a wholly empty profile is admissible
            // only beside a usable sibling, and anything else is refused for
            // the half it lost.
            return Ok(structural_eviction(directory, state, waive_empty));
        }
        return Ok(Some(Eviction {
            profile: directory.to_path_buf(),
            reason: None,
            missing,
        }));
    }
    // No stamp section, so only the structural answer is available, and it is
    // the one a successful target-selected build can make look like damage.
    Ok(structural_eviction(directory, state, waive_empty))
}

/// The structural answer: refuse a profile whose layout is missing a half.
///
/// A profile holding neither half is what a target-selected build leaves where
/// the host has no units, and it is admissible exactly when a sibling profile
/// holds the tree — that is what `waive_empty` records. Anything else with an
/// incomplete layout is refused for the half it lost.
///
/// Shared by the two paths that reach it: a directory the stamp says nothing
/// about, and one whose section names nothing and so cannot refuse anything
/// through the exact comparison.
fn structural_eviction(directory: &Path, state: &State, waive_empty: bool) -> Option<Eviction> {
    if waive_empty && whole_empty_profile(state) {
        return None;
    }
    state
        .empty_layout_reason()
        .map(|reason| Eviction::whole(directory, reason))
}

/// Whether `state` is a directory holding neither half of Cargo's layout.
fn whole_empty_profile(state: &State) -> bool {
    state
        .layout()
        .is_some_and(|layout| layout.units.is_empty() && layout.fingerprints.is_empty())
}

/// Whether some named profile still holds a tree a timed assertion could reuse.
///
/// This is the condition that lets a target-selected preparation exist at all.
/// A build selected onto a target triple writes the fixture's units beneath the
/// triple and leaves the host profile with only lock files and `build/`
/// whenever nothing in the graph needs a host unit — measured on a
/// dependency-free crate, and the state that refused every such preparation
/// before this rule. What justifies waiving an empty profile is not its own
/// shape but its sibling holding the tree, so an empty profile is only ever
/// excused beside one that is populated.
///
/// Nothing else is waived. A profile that lost *half* its layout still refuses,
/// because that is what a partial clean leaves, and a stamp recording
/// something that has gone still refuses through the exact layer.
fn any_usable(states: &[State]) -> bool { states.iter().any(State::is_usable) }

/// The artefact directories the stamp vouches for that are no longer usable.
///
/// Shared by the gate and its failure message, so "the gate refused" and
/// "the message names why" can never disagree.
///
/// `stamp` is the matching stamp, when one was found. Without it there is
/// nothing below the directories to check, and the structural layer alone
/// decides. With it, every directory [`Preparation::artefacts`] names must
/// have a section of its own: a stamp describing fewer directories than the
/// preparation names cannot vouch for the extra ones, and a section-count
/// mismatch is treated as the whole directory being unvouched for rather than
/// silently skipping the named-artefact layer.
///
/// # Errors
///
/// Returns the read error when any directory cannot be inspected.
pub(super) fn evicted(
    preparation: &Preparation,
    stamp: Option<&Stamp>,
) -> io::Result<Vec<Eviction>> {
    let mut states = Vec::with_capacity(preparation.artefacts.len());
    for (index, directory) in preparation.artefacts.iter().enumerate() {
        let recorded = stamp.and_then(|stamp| stamp.recorded.get(index));
        // With a stamp in hand, a directory it says nothing about is
        // unvouched for: a stamp carrying fewer sections than the
        // preparation names describes some other preparation, and the
        // directories it omits must not be waved through by the structural
        // layer alone.
        let vouched = stamp.is_none() || recorded.is_some();
        states.push(state::inspect(directory, vouched)?);
    }
    let waive_empty = any_usable(&states);
    let mut evictions = Vec::new();
    // Zipped rather than indexed: one state was pushed per directory above, so
    // the two run in step, and the pairing is what the zip states rather than
    // an invariant a reader has to recover from the earlier loop.
    for ((index, directory), directory_state) in
        preparation.artefacts.iter().enumerate().zip(&states)
    {
        let recorded = stamp.and_then(|stamp| stamp.recorded.get(index));
        let profile = ProfileUnderJudgement {
            directory,
            state: directory_state,
            recorded: recorded.map(Vec::as_slice),
            waive_empty,
        };
        if let Some(eviction) = eviction_of(&profile, &preparation.owned_packages)? {
            evictions.push(eviction);
        }
    }
    Ok(evictions)
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

#[cfg(test)]
mod tests;
