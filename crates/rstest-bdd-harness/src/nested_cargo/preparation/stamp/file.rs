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
    fingerprint::Fingerprint,
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
