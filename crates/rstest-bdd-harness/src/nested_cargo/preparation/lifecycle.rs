//! The gate that decides whether a family is prepared, and the build that
//! follows when it is not.
//!
//! Three entry points reach this module. [`ensure_prepared`] reads the runner
//! decision from the process environment; [`ensure_prepared_with`] and
//! [`ensure_prepared_with_probe`] take it — and the toolchain probe — as
//! parameters so the protocol can be exercised without mutating the
//! environment or spawning `cargo -vV`. [`prepare_now`] is the unconditional
//! form the named preparation test uses.
//!
//! The policy lives here rather than in the stamp module because the two
//! runners want opposite things from the same fact. An unreadable stamp is
//! fail-fast evidence under `nextest`, where the tree is present but cannot be
//! vouched for, and merely a rebuild under `cargo test`. Keeping that choice
//! out of the stamp query is what lets the query stay a plain read.

use std::{io, path::Path, time::Instant};

use super::{
    Preparation,
    PreparationSummary,
    artefacts,
    stamp::{
        Fingerprint,
        Stamp,
        fingerprint_with,
        matching_stamp,
        missing_stamp_message,
        note,
        render,
        stamp_path,
        unreadable_stamp_error,
        write_stamp,
    },
    toolchain::toolchain_identity,
};

/// Ensure the family's artefacts are prepared, running `prepare` if needed.
///
/// A stamp that matches the current fingerprint returns immediately. A missing
/// or stale stamp under `cargo nextest` returns a [`io::ErrorKind::NotFound`]
/// error naming the preparation command instead of preparing inside a timed
/// test. Under plain `cargo test` the closure runs under a process-wide lock,
/// and the stamp is written only after it succeeds.
///
/// # Errors
///
/// Returns the closure's error when preparation fails, and the I/O error when
/// the stamp cannot be read or written.
pub fn ensure_prepared<F>(preparation: &Preparation, prepare: F) -> io::Result<()>
where
    F: FnOnce() -> io::Result<PreparationSummary>,
{
    ensure_prepared_with(preparation, super::super::running_under_nextest(), prepare)
}

/// [`ensure_prepared`] with the runner decision supplied.
///
/// Exposed as hidden API so the protocol can be tested — here and in the
/// workspace suites that gate on it — without mutating the process
/// environment, which would race every other test in the same binary.
///
/// # Errors
///
/// As [`ensure_prepared`].
#[doc(hidden)]
pub fn ensure_prepared_with<F>(
    preparation: &Preparation,
    under_nextest: bool,
    prepare: F,
) -> io::Result<()>
where
    F: FnOnce() -> io::Result<PreparationSummary>,
{
    ensure_prepared_with_probe(preparation, under_nextest, toolchain_identity, prepare)
}

/// [`ensure_prepared_with`] with the toolchain probe supplied.
///
/// The probe is a parameter for the same reason the runner decision is: a test
/// cannot make `cargo -vV` fail, so a boundary that swallowed the probe's error
/// would be unreachable from any test that only used the real one. Everything
/// else is unchanged — the probe is called exactly once, and its error stops
/// the gate before any stamp is read.
///
/// # Errors
///
/// As [`ensure_prepared`], plus the probe's own error.
#[doc(hidden)]
pub fn ensure_prepared_with_probe<P, F>(
    preparation: &Preparation,
    under_nextest: bool,
    probe: P,
    prepare: F,
) -> io::Result<()>
where
    P: FnOnce() -> io::Result<String>,
    F: FnOnce() -> io::Result<PreparationSummary>,
{
    let fingerprint = fingerprint_with(preparation, probe)?;
    let stamp = stamp_path(preparation);
    // An unreadable stamp is neither a match nor an absence, and the two
    // runners reach opposite conclusions from it. Under `nextest` it is
    // fail-fast evidence — the tree is present but cannot be vouched for, which
    // a "missing or stale" message would misreport. Under `cargo test` the
    // caller prepares again, so the read failure only costs a rebuild.
    let matching = match matching_stamp(&stamp, &fingerprint) {
        Ok(matching) => matching,
        Err(error) if !under_nextest => {
            note(&format!("stamp {} unreadable ({error})", stamp.display()));
            None
        }
        Err(error) => return Err(unreadable_stamp_error(&stamp, &error)),
    };
    // Both halves are required: a stamp vouches for the inputs, and the tree
    // must still hold the units it recorded.
    if matching.is_some() && artefacts::all_present(preparation, matching.as_ref())? {
        note(&format!(
            "family={} is prepared ({})",
            preparation.family,
            stamp.display()
        ));
        return Ok(());
    }

    if under_nextest {
        return Err(io::Error::new(
            io::ErrorKind::NotFound,
            missing_stamp_message(preparation, &stamp, matching.as_ref())?,
        ));
    }

    prepare_locked(preparation, &stamp, &fingerprint, prepare)
}

/// Prepare the family unconditionally, for the named preparation test.
///
/// This is the entry point `make prepare-nested-fixtures` reaches: it runs
/// under nextest, where [`ensure_prepared`] would otherwise fail fast. It is
/// still idempotent, so a warm target directory costs only the stamp read.
///
/// # Errors
///
/// As [`ensure_prepared`].
pub fn prepare_now<F>(preparation: &Preparation, prepare: F) -> io::Result<()>
where
    F: FnOnce() -> io::Result<PreparationSummary>,
{
    prepare_now_with_probe(preparation, toolchain_identity, prepare)
}

/// [`prepare_now`] with the toolchain probe supplied.
///
/// # Errors
///
/// As [`prepare_now`], plus the probe's own error.
#[doc(hidden)]
pub fn prepare_now_with_probe<P, F>(
    preparation: &Preparation,
    probe: P,
    prepare: F,
) -> io::Result<()>
where
    P: FnOnce() -> io::Result<String>,
    F: FnOnce() -> io::Result<PreparationSummary>,
{
    let fingerprint = fingerprint_with(preparation, probe)?;
    prepare_locked(preparation, &stamp_path(preparation), &fingerprint, prepare)
}

/// Run `prepare` under the process-wide lock unless already prepared, then stamp.
fn prepare_locked<F>(
    preparation: &Preparation,
    stamp: &Path,
    fingerprint: &Fingerprint,
    prepare: F,
) -> io::Result<()>
where
    F: FnOnce() -> io::Result<PreparationSummary>,
{
    let _guard = super::lock();
    // Another process or thread may have prepared the family while this one
    // waited for the lock. The artefacts are re-checked on the same terms as
    // the gate: a stamp whose tree was evicted while this process waited is
    // not a prepared family.
    //
    // An unreadable stamp is logged and then prepared over rather than
    // propagated. Both callers reach here having already decided to build —
    // `prepare_now`'s contract is to prepare unconditionally, and the lazy
    // path is only taken after `ensure_prepared_with` chose to re-prepare — so
    // refusing here would leave a corrupt stamp with no remedy but deleting
    // the tree by hand. What was wrong before was the silence: `.ok()` made an
    // unreadable stamp indistinguishable from a warm one that had merely
    // missed, and the rebuild happened with no explanation. The wording names
    // the second read so this line is not mistaken for the pre-lock one.
    let matching: Option<Stamp> = match matching_stamp(stamp, fingerprint) {
        Ok(matching) => matching,
        Err(error) => {
            note(&format!(
                "stamp {} unreadable after waiting for the lock ({error}); preparing again",
                stamp.display()
            ));
            None
        }
    };
    if matching.is_some() && artefacts::all_present(preparation, matching.as_ref())? {
        return Ok(());
    }

    let started = Instant::now();
    let summary = prepare()?;
    // Structural check first: a command can exit zero while leaving no profile
    // at all, and asking for a record before that is known would report a bare
    // filesystem error instead of the missing-directory diagnosis.
    artefacts::verify_structural(preparation)?;
    // Then record what the command left, and hold it to the same record. The
    // artefacts can be gone again — a concurrent `cargo clean`, an interrupted
    // build. Stamping that state would advertise a prepared family whose next
    // timed assertion pays for the rebuild, so the missing names are reported
    // instead.
    let recorded = artefacts::record(preparation)?;
    artefacts::verify_recorded(preparation, &recorded)?;
    write_stamp(stamp, &render(fingerprint, &recorded))?;
    note(&format!(
        "family={} prepared in {:.1}s ({} Compiling lines); stamp {}",
        preparation.family,
        started.elapsed().as_secs_f64(),
        summary.compiled_units,
        stamp.display()
    ));
    Ok(())
}
