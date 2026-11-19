//! The gate that decides whether a family is prepared, and the build that
//! follows when it is not.
//!
//! Four entry points reach this module. [`ensure_prepared`] reads the runner
//! decision from the process environment; [`ensure_prepared_with`] takes that
//! decision as a parameter; [`ensure_prepared_reported`] takes the injected
//! boundaries as well, so the protocol can be exercised without mutating the
//! environment or spawning `cargo -vV`. [`prepare_now`] is the unconditional
//! form the named preparation test uses, and [`prepare_now_reported`] is its
//! injectable twin.
//!
//! The probe and the reporter travel together in [`Instruments`] rather than as
//! two more parameters. Every caller that overrides one of them overrides the
//! other at the same time, and threading them separately made the innermost
//! boundary take five arguments — past the count this repository's lints allow
//! and past what a reader can keep in view. The entry point that supplies the
//! real pair binds the probe to the preparation's own environment, so the
//! compiler identified is the one the nested build will run, and binds the
//! reporter to `stamp::note`, so production diagnostics are unchanged.
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
    ensure_prepared_reported(
        preparation,
        under_nextest,
        instruments_of(preparation),
        prepare,
    )
}

/// The two boundaries a caller may substitute for the real ones.
///
/// The probe resolves the compiler's identity and the reporter carries this
/// module's diagnostics. They travel together because they are overridden for
/// the same purpose and at the same time: a test that cannot make `cargo -vV`
/// fail and cannot read stderr needs both injected, and a caller that supplies
/// one of them wants the other's production binding. Grouping them also keeps
/// the innermost gate inside the argument budget this repository enforces.
///
/// The asynchronous forms are the ones the gate takes: the probe is called
/// exactly once, and its error stops the gate before any stamp is read.
pub struct Instruments<P, R> {
    /// Resolves the toolchain identity the fingerprint is built from.
    pub probe: P,
    /// Receives every diagnostic the gate emits, as a finished message.
    pub report: R,
}

impl<P, R> Instruments<P, R> {
    /// Pair a probe with a reporter.
    ///
    /// # Parameters
    /// - `probe`: The toolchain probe; called once per gate.
    /// - `report`: Where diagnostics go; called at most twice per gate, once before the lock is
    ///   taken and once after.
    ///
    /// # Returns
    /// The two grouped, in the field order the gate takes them.
    #[must_use]
    pub fn new(probe: P, report: R) -> Self { Self { probe, report } }
}

/// Report a preparation diagnostic the way production does.
///
/// Resolved as a function value rather than written inline at each call, so
/// the boundary below, `invalidate`'s and the production entry points share one
/// binding: a second spelling of `stamp::note` would let the two diverge
/// without any test noticing. Shared with the sibling module rather than
/// duplicated there for that reason.
pub(super) fn reporter_of() -> impl FnMut(&str) { stamp_note }

/// Bind the real toolchain probe to the preparation's own environment.
///
/// Every entry point that supplies the real probe needs exactly this binding,
/// and it must be the preparation's environment rather than the process's: the
/// compiler identified has to be the one the nested build will run.
fn probe_of(preparation: &Preparation) -> impl FnOnce() -> io::Result<String> + '_ {
    move || toolchain_identity(&preparation.environment, &preparation.working_directory)
}

/// The real boundaries, bound to `preparation`.
///
/// One binding, used by both production entry points, so the compiler the gate
/// identifies and the reporter it writes through cannot drift apart.
pub(super) fn instruments_of(
    preparation: &Preparation,
) -> Instruments<impl FnOnce() -> io::Result<String> + '_, impl FnMut(&str)> {
    Instruments::new(probe_of(preparation), reporter_of())
}

/// [`ensure_prepared_with`] with the diagnostic reporter supplied.
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
    instruments: Instruments<P, R>,
    prepare: F,
) -> io::Result<()>
where
    P: FnOnce() -> io::Result<String>,
    R: FnMut(&str),
    F: FnOnce() -> io::Result<PreparationSummary>,
{
    let Instruments { probe, mut report } = instruments;
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

    prepare_locked(
        preparation,
        &Target {
            stamp: &stamp,
            fingerprint: &fingerprint,
        },
        report,
        prepare,
    )
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
    let fingerprint = fingerprint_with(preparation, probe)?;
    prepare_locked(
        preparation,
        &Target {
            stamp: &stamp_path(preparation),
            fingerprint: &fingerprint,
        },
        report,
        prepare,
    )
}

/// The stamp and fingerprint a preparation is being judged against.
///
/// They are produced together and only ever read together, and grouping them
/// keeps [`prepare_locked`] inside this repository's argument budget. The
/// struct is borrowed rather than moved for the same reason its fields are
/// references: it is a view of two values the caller owns, not a value in its
/// own right, and moving it would only copy two pointers.
struct Target<'a> {
    /// Where the stamp under judgement lives.
    stamp: &'a Path,
    /// What that stamp must record for the tree to count as prepared.
    fingerprint: &'a Fingerprint,
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
    target: &Target<'_>,
    mut report: R,
    prepare: F,
) -> io::Result<()>
where
    R: FnMut(&str),
    F: FnOnce() -> io::Result<PreparationSummary>,
{
    let Target { stamp, fingerprint } = target;
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
