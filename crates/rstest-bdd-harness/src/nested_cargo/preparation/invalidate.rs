//! Withdrawing a family's prepared claim.
//!
//! A stamp is the only thing that decides whether a later attempt builds at
//! all, because every entry point in [`lifecycle`](super::lifecycle) returns
//! early from a matching one. This module is the one caller that can refute
//! that match from outside: it exists for a caller whose own completeness rule
//! is stricter than the stamp can be, so the next attempt is a build rather
//! than the same refusal again.
//!
//! It lives apart from the lifecycle because it is not part of the gate's
//! decision — nothing here reads a fingerprint, consults the runner or runs a
//! preparation — and because the two spellings of its diagnostic belong
//! together.

use std::io;

use super::{Preparation, lifecycle::reporter_of, stamp::stamp_path};

/// Remove the stamp that advertises `preparation` as prepared.
///
/// Both [`prepare_now`](super::prepare_now) and the lazy `cargo test` path of
/// [`ensure_prepared_building`](super::ensure_prepared_building) return early from a matching
/// stamp, so the stamp is what decides whether a later attempt builds at all.
/// This entry point exists for a caller whose own completeness rule is stricter
/// than the stamp can be: `Preparation::artefacts` answers "the units the
/// preparation recorded are still there", and cannot answer "they are the units
/// this family needs", because only the caller knows which ones those are. A
/// caller that checks its own witness *after* the gate admitted the stamp can
/// therefore find the stamp vouching for a tree its assertions cannot use — and
/// if it simply reports that, every later attempt is refused the same way
/// without the preparation ever running again.
///
/// Removing the stamp is what turns the next attempt into a build. It does not
/// repair the tree: the caller reports the refusal it found, and the recovery
/// is the next run's.
///
/// A stamp that is already absent is not an error. The caller is withdrawing a
/// claim it has just refuted, and whether that claim was ever written is not
/// its business; reporting a missing file here would replace the useful failure
/// (the witness) with a filesystem one.
///
/// # Errors
///
/// Returns the removal error when the stamp exists and cannot be removed.
pub fn invalidate(preparation: &Preparation) -> io::Result<()> {
    invalidate_reported(preparation, reporter_of())
}

/// [`invalidate`] with the diagnostic reporter supplied.
///
/// The reporter is a parameter for the reason
/// [`ensure_prepared_building_reported`](super::ensure_prepared_building_reported) gives: a
/// message written straight to stderr is not observable by a test, and this
/// one describes a claim being withdrawn, which is worth asserting.
///
/// # Parameters
/// - `preparation`: The family whose stamp is removed.
/// - `report`: Where the diagnostic goes; called once when a stamp is removed, and not at all when
///   there was none.
///
/// # Returns
/// `Ok(())` once no stamp describes `preparation`.
///
/// # Errors
///
/// As [`invalidate`].
#[doc(hidden)]
pub fn invalidate_reported<R: FnMut(&str)>(
    preparation: &Preparation,
    mut report: R,
) -> io::Result<()> {
    let stamp = stamp_path(preparation);
    match std::fs::remove_file(&stamp) {
        Ok(()) => {
            report(&format!("removed the stamp {}", stamp.display()));
            Ok(())
        }
        Err(error) if error.kind() == io::ErrorKind::NotFound => Ok(()),
        Err(error) => Err(error),
    }
}
