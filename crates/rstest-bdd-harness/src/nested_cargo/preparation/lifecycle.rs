//! The gate that decides whether a family is prepared, and the build that
//! follows when it is not.
//!
//! Five entry points reach this module. [`ensure_prepared`] reads the runner
//! decision from the process environment; [`ensure_prepared_with`],
//! [`ensure_prepared_with_probe`] and [`ensure_prepared_reported`] take it —
//! and the toolchain probe and the message reporter — as parameters so the
//! protocol can be exercised without mutating the environment or spawning
//! `cargo -vV`. [`prepare_now`] is the unconditional form the named preparation
//! test uses. The entry points that supply the real probe bind it to the
//! preparation's own environment, so the compiler identified is the one the
//! nested build will run, and those that supply the real reporter bind it to
//! `stamp::note`, so production diagnostics are unchanged.
//!
//! The reporter is a parameter at the innermost boundary rather than a direct
//! write to stderr, because the diagnostics are part of what this module
//! promises: a corrupt stamp is explained and prepared over, not silently
//! rebuilt. A message written straight to stderr is invisible to any test that
//! does not capture the entire process's output, so the promise would be
//! untestable.
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
        note as stamp_note,
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
/// The two runners do not report the same set, because they make opposite
/// decisions about the same evidence.
///
/// Under `cargo nextest` this returns the probe's error, the I/O error when the
/// stamp cannot be read, and the [`io::ErrorKind::NotFound`] refusal for a
/// missing or stale stamp — including the failure to render that refusal's
/// message. It never runs `prepare`, so it never returns the closure's error.
///
/// Under plain `cargo test` a read failure is deliberately *not* reported: it
/// is logged and the family is prepared again, because a corrupt stamp would
/// otherwise have no remedy but deleting the tree by hand. What this returns
/// there is the probe's error, the closure's error when preparation fails, the
/// I/O error when the prepared artefacts cannot be checked, and the I/O error
/// when the stamp cannot be written.
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
    ensure_prepared_with_probe(preparation, under_nextest, probe_of(preparation), prepare)
}

/// Bind the real toolchain probe to the preparation's own environment.
///
/// Every entry point that supplies the real probe needs exactly this binding,
/// and it must be the preparation's environment rather than the process's: the
/// compiler identified has to be the one the nested build will run.
fn probe_of(preparation: &Preparation) -> impl FnOnce() -> io::Result<String> + '_ {
    move || toolchain_identity(&preparation.environment, &preparation.working_directory)
}

/// Report a preparation diagnostic the way production does.
///
/// Resolved as a function value rather than written inline at each call, so
/// the boundary below, `invalidate`'s and the production entry points share one
/// binding: a second spelling of `stamp::note` would let the two diverge
/// without any test noticing. Shared with the sibling module rather than
/// duplicated there for that reason.
pub(super) fn reporter_of() -> impl FnMut(&str) { stamp_note }

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
    ensure_prepared_reported(preparation, under_nextest, probe, reporter_of(), prepare)
}

/// [`ensure_prepared_with_probe`] with the diagnostic reporter supplied.
///
/// The reporter is a parameter for the same reason the runner decision and the
/// probe are: the diagnostics this module emits are a promise about what a
/// developer sees when a stamp is corrupt under `cargo test`, and a message
/// written straight to stderr cannot be observed by a test without capturing
/// the whole process's output. Taking it here lets a case assert both halves of
/// that promise — that the message names what was wrong, and that the family is
/// prepared again afterwards — against the boundary production actually runs.
///
/// It is called at most twice per gate, once before the lock is taken and once
/// after, and receives the finished message rather than any of its parts.
///
/// # Errors
///
/// As [`ensure_prepared`], plus the probe's own error.
#[doc(hidden)]
pub fn ensure_prepared_reported<P, R, F>(
    preparation: &Preparation,
    under_nextest: bool,
    probe: P,
    mut report: R,
    prepare: F,
) -> io::Result<()>
where
    P: FnOnce() -> io::Result<String>,
    R: FnMut(&str),
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
            report(&format!("stamp {} unreadable ({error})", stamp.display()));
            None
        }
        Err(error) => return Err(unreadable_stamp_error(&stamp, &error)),
    };
    // Both halves are required: a stamp vouches for the inputs, and the tree
    // must still hold the units it recorded.
    if matching.is_some() && artefacts::all_present(preparation, matching.as_ref())? {
        report(&format!(
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

    prepare_locked(preparation, &stamp, &fingerprint, report, prepare)
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
    prepare_now_with_probe(preparation, probe_of(preparation), prepare)
}

/// [`prepare_now`] with the diagnostic reporter supplied.
///
/// Split so the unconditional path's diagnostics are observable too: a warm
/// family is reported as prepared and a cold one names the units it compiled,
/// both of which are the evidence that cold work left the timed assertions.
///
/// # Parameters
/// - `preparation`: The family to prepare.
/// - `probe`: The toolchain probe the fingerprint is built from.
/// - `report`: Where a diagnostic goes; called once per message.
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
    probe: P,
    report: R,
    prepare: F,
) -> io::Result<()>
where
    P: FnOnce() -> io::Result<String>,
    R: FnMut(&str),
    F: FnOnce() -> io::Result<PreparationSummary>,
{
    let fingerprint = fingerprint_with(preparation, probe)?;
    prepare_locked(
        preparation,
        &stamp_path(preparation),
        &fingerprint,
        report,
        prepare,
    )
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
    prepare_now_reported(preparation, probe, reporter_of(), prepare)
}

/// Run `prepare` under the process's preparation mutex unless already prepared,
/// then stamp.
///
/// The mutex serializes the threads of one test process, which is what plain
/// `cargo test` has; it is not a cross-process lock, and under nextest the
/// `cargo-spawning` test group is what keeps two nested builds apart.
///
/// `report` receives every diagnostic this function emits, including the
/// post-lock read failure explained below.
fn prepare_locked<R, F>(
    preparation: &Preparation,
    stamp: &Path,
    fingerprint: &Fingerprint,
    mut report: R,
    prepare: F,
) -> io::Result<()>
where
    R: FnMut(&str),
    F: FnOnce() -> io::Result<PreparationSummary>,
{
    let _guard = super::lock();
    // Another thread may have prepared the family while this one waited for
    // the mutex, and a timed assertion may have evicted the tree while the
    // build ran. The artefacts are re-checked on the same terms as the gate: a
    // stamp whose tree was evicted while this thread waited is not a prepared
    // family.
    //
    // An unreadable stamp is logged and then prepared over rather than
    // propagated. Both callers reach here having already decided to build —
    // `prepare_now`'s contract is to prepare unconditionally, and the lazy
    // path is only taken after `ensure_prepared_reported` chose to re-prepare —
    // so refusing here would leave a corrupt stamp with no remedy but deleting
    // the tree by hand. What was wrong before was the silence: `.ok()` made an
    // unreadable stamp indistinguishable from a warm one that had merely
    // missed, and the rebuild happened with no explanation. The wording names
    // the second read so this line is not mistaken for the pre-lock one.
    let matching: Option<Stamp> = match matching_stamp(stamp, fingerprint) {
        Ok(matching) => matching,
        Err(error) => {
            report(&format!(
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
    // And hold the caller's declaration to the same record. `owned_packages`
    // restricts what the stamp may speak for, so a package named there that the
    // build never produced contributes nothing and its later eviction would
    // pass. See `artefacts::verify_declared`.
    artefacts::verify_declared(preparation, &recorded)?;
    write_stamp(stamp, &render(fingerprint, &recorded))?;
    report(&format!(
        "family={} prepared in {:.1}s ({} Compiling lines); stamp {}",
        preparation.family,
        started.elapsed().as_secs_f64(),
        summary.compiled_units,
        stamp.display()
    ));
    Ok(())
}
