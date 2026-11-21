//! The unconditional preparation entry points.
//!
//! These are reached by `make prepare-nested-fixtures` and by the named
//! `nested_fixture_preparation` tests, which must build even under nextest
//! where the lazy gate would fail fast. They share the locking, re-read and
//! stamping that follow a build; the difference from the lazy path is only
//! that they never let the stamp answer skip the build.
//!
//! They live beside the gate rather than in it so neither module exceeds the
//! repository's per-file budget, and both are re-exported from
//! [`super`] so no call site names this path.

use std::io;

use super::{Target, prepare_locked};
use crate::nested_cargo::preparation::{
    Preparation,
    PreparationSummary,
    lifecycle::{Instruments, instruments_of},
    stamp::{fingerprint_with, stamp_path},
};

/// Prepare the family unconditionally, for the named preparation test.
///
/// This is the entry point `make prepare-nested-fixtures` reaches: it runs
/// under nextest, where [`ensure_prepared_building`] would otherwise fail fast
/// on a family that was never prepared. It is still idempotent, so a warm
/// target directory costs the toolchain probes and the stamp and artefact
/// checks under the lock, and no build.
///
/// # Errors
///
/// As [`ensure_prepared_building`].
///
/// [`ensure_prepared_building`]: super::ensure_prepared_building
pub fn prepare_now<F>(preparation: &Preparation, prepare: F) -> io::Result<()>
where
    F: FnOnce() -> io::Result<PreparationSummary>,
{
    prepare_now_reported(preparation, instruments_of(preparation), prepare)
}

/// [`prepare_now`] with the probe and reporter supplied.
///
/// Split so the unconditional path's diagnostics and compiler identity are
/// observable too: a warm family is reported as prepared and a cold one names
/// the units it compiled, both of which are the evidence that cold work left
/// the timed assertions.
///
/// # Parameters
/// - `preparation`: The family to prepare.
/// - `instruments`: The toolchain probe the fingerprint is built from and where diagnostics go; see
///   [`Instruments`].
/// - `prepare`: The build to run when the family is not already prepared.
///
/// # Returns
/// `Ok(())` once the family is prepared or was already, and a stamp written
/// after a build.
///
/// # Errors
///
/// As [`prepare_now`], plus the probe's own error.
#[doc(hidden)]
pub fn prepare_now_reported<P, R, F>(
    preparation: &Preparation,
    instruments: Instruments<P, R>,
    prepare: F,
) -> io::Result<()>
where
    P: FnOnce() -> io::Result<String>,
    R: FnMut(&str),
    F: FnOnce() -> io::Result<PreparationSummary>,
{
    let Instruments { probe, report } = instruments;
    // The unconditional form declines to act on the stamp answer, so only the
    // fingerprint is composed here. `prepare_locked` then reads the stamp under
    // the lock, which is where a decision made before the lock would be stale.
    let fingerprint = fingerprint_with(preparation, probe)?;
    let stamp = stamp_path(preparation);
    prepare_locked(
        preparation,
        &Target {
            stamp: &stamp,
            fingerprint: &fingerprint,
        },
        report,
        prepare,
    )
    .map(|_outcome| ())
}
