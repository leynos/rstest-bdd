//! Where a stamp lives and what reading it back means.
//!
//! This module only reads. It names the file a family writes, opens it, and
//! reports what it found — a match, an absence, or a read failure. Deciding
//! what any of those means for a run belongs to the caller, because the two
//! runners reach opposite conclusions from the same answer.

use std::{
    fs,
    io,
    path::{Path, PathBuf},
};

use super::{
    super::Preparation,
    fingerprint::{Fingerprint, fingerprint_with},
    format::{PREPARED_FILE_SUFFIX, Stamp, parse},
};

/// The stamp file protecting `preparation.directory`.
pub(in crate::nested_cargo::preparation) fn stamp_path(preparation: &Preparation) -> PathBuf {
    preparation
        .directory
        .join(format!(".{}{}", preparation.family, PREPARED_FILE_SUFFIX))
}

/// The stamp at `stamp` if it was written for exactly `fingerprint`.
///
/// Returns `Ok(None)` for a missing stamp and for one whose fingerprint
/// differs. Both are "not prepared", which is the safe answer: the caller
/// prepares again or fails fast rather than admitting a timed assertion
/// against a tree the stamp does not describe.
///
/// An unreadable stamp is neither a match nor an absence, and the two runners
/// reach opposite conclusions from it: under `nextest` a non-match fails the
/// test, while under `cargo test` it re-prepares. Deciding that here would
/// force the `nextest` case to report a missing stamp for a tree that is
/// present but unreadable — a diagnosis the reader cannot act on. Returning the
/// error keeps the choice, and the cause, at the boundary that owns the runner
/// policy.
///
/// This query is silent by design. It reports what it found and nothing else,
/// so the boundary that applies the runner policy is also the one place that
/// decides what to log. Logging here as well would write the same failure twice
/// on the `cargo test` path, and would put a side effect on a function whose
/// contract is otherwise just a read.
///
/// # Errors
///
/// Returns the read error when the stamp exists but cannot be read. Treating
/// that as "not prepared" would silently discard a stamp that may well
/// describe a warm tree, and the caller would pay for the rebuild with no
/// indication of why.
pub(in crate::nested_cargo::preparation) fn matching_stamp(
    stamp: &Path,
    fingerprint: &Fingerprint,
) -> io::Result<Option<Stamp>> {
    let text = match fs::read_to_string(stamp) {
        Ok(text) => text,
        Err(error) if error.kind() == io::ErrorKind::NotFound => return Ok(None),
        Err(error) => return Err(error),
    };
    Ok(parse(&text, fingerprint))
}

/// What reading the stamp found, before any runner policy is applied.
///
/// This is the raw answer [`matching_stamp`] gives, named as a value so the
/// admission query and the gate can share one comparison instead of each
/// spelling out "build the fingerprint, read the stamp, tell a match from an
/// absence from a read failure". It does not decide anything: what a missing
/// stamp means, and what an unreadable one means, are the runners' business
/// and stay at the boundary that owns the policy.
#[derive(Debug)]
pub(in crate::nested_cargo::preparation) enum StampDecision {
    /// The stamp was written for exactly this fingerprint.
    Matching(Stamp),
    /// No stamp file is there.
    Missing,
    /// A stamp is there but its fingerprint differs, so it describes an
    /// earlier build of other inputs.
    Stale,
    /// The stamp exists but cannot be read.
    Unreadable(io::Error),
}

/// A fingerprint and the stamp answer read against it.
///
/// They travel together because the fingerprint is what makes a stamp matching
/// or stale, and a caller that later writes a stamp needs the same fingerprint
/// it judged against. Returning only the decision would force that caller to
/// recompose it, and two compositions of one value are two chances to differ.
#[derive(Debug)]
pub(in crate::nested_cargo::preparation) struct StampReading {
    /// What a stamp must record for the tree to count as prepared.
    pub(in crate::nested_cargo::preparation) fingerprint: Fingerprint,
    /// What the stamp at `stamp_path` recorded.
    pub(in crate::nested_cargo::preparation) decision: StampDecision,
}

/// Resolve the toolchain and read the stamp against the fingerprint it yields.
///
/// The one place the gate and the admission query agree on what "the stamp
/// says" means, so
/// [`admit`](super::super::lifecycle::admit) admitting a tree and
/// the gate returning at the stamp cannot diverge. Reading a stamp is the whole
/// job: the artefact inspection that follows a match is the caller's, because
/// only the caller knows whether an eviction is a refusal or a reason to
/// rebuild.
///
/// A read failure is an answer here rather than an error, because it is a fact
/// about the tree the caller asked about. What it means for the run — fail fast
/// under Nextest, prepare over under `cargo test` — stays at the boundary that
/// owns the runner policy.
///
/// # Errors
///
/// Returns the probe's error, so a toolchain that cannot be identified stops
/// the caller before any stamp is read.
pub(in crate::nested_cargo::preparation) fn read_stamp<P>(
    preparation: &Preparation,
    probe: P,
) -> io::Result<StampReading>
where
    P: FnOnce() -> io::Result<String>,
{
    let fingerprint = fingerprint_with(preparation, probe)?;
    let stamp = stamp_path(preparation);
    let decision = match matching_stamp(&stamp, &fingerprint) {
        Ok(Some(found)) => StampDecision::Matching(found),
        // `matching_stamp` returns `None` both for an absent file and for one
        // whose fingerprint differs. Distinguishing them matters to a caller
        // explaining a refusal, and the file's presence is the whole
        // difference, so it is read once more rather than guessed at.
        Ok(None) if stamp.exists() => StampDecision::Stale,
        Ok(None) => StampDecision::Missing,
        Err(error) => StampDecision::Unreadable(error),
    };
    Ok(StampReading {
        fingerprint,
        decision,
    })
}
