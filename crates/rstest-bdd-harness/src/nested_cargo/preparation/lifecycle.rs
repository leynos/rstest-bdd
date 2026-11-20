//! The gate that decides whether a family is prepared, and the build that
//! follows when it is not.
//!
//! The gate is split by what its names promise, because the two questions a
//! caller can ask are not the same question and must not be reachable through
//! the same name.
//!
//! [`admit`] is the read-only admission query. It takes the runner decision
//! explicitly, performs no build, writes no stamp and takes no lock, and
//! answers with [`Admission`] — a value that states whether the tree is
//! reusable. It lives in [`admission`], beside the injectable
//! [`admit_with_probe`] the tests drive.
//!
//! [`ensure_prepared_building`] is the build-capable entry point. Its name says
//! that it may prepare, and its [`PreparationOutcome`] return value says
//! whether a build actually ran. That entry point and [`prepare_now`] both take
//! the runner decision as a parameter; [`ensure_prepared_building_reported`]
//! and [`prepare_now_reported`] take the injected boundaries as well, so the
//! protocol can be exercised without mutating the environment or spawning
//! `cargo -vV`.
//!
//! [`prepare_now`] is the unconditional form the named preparation test uses,
//! and [`prepare_now_reported`] is its injectable twin.
//!
//! No public entry point in this module reads the process environment. A
//! caller that needs the runner decision supplies it, which is what makes the
//! environment read a decision at the call site rather than an implicit one
//! inside a gate whose name does not mention it.
//!
//! The probe and the reporter travel together in [`Instruments`] rather than as
//! two more parameters. Every caller that overrides one of them overrides the
//! other at the same time, and threading them separately made the innermost
//! boundary take five arguments — past the count this repository's lints allow
//! and past what a reader can keep in view. The entry point that supplies the
//! real pair binds the probe to the preparation's own environment, so the
//! compiler identified is the one the nested build will run, and binds the
//! reporter to the stderr writer [`instruments`] owns, so production
//! diagnostics are unchanged.
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
//! out of the stamp query is what lets the query stay a read.
//!
//! The same boundary runs the directory inspection that a refusal's message
//! names. [`missing_stamp_message`] takes the result rather than gathering it,
//! which is what keeps that builder pure and testable without a filesystem.
//!
//! # Modules
//!
//! [`instruments`] holds the injected boundaries and their production
//! bindings, and [`admission`] holds the read-only query. Both are re-exported
//! below, so callers keep naming `lifecycle::` and no call site had to move.

use std::{io, path::Path, time::Instant};

mod admission;
mod instruments;
mod prepared;

pub use admission::{Admission, UnreadableStamp, admit, admit_with_probe};
pub use instruments::Instruments;
pub(in crate::nested_cargo::preparation) use instruments::{instruments_of, reporter_of};
pub use prepared::{prepare_now, prepare_now_reported};

use super::{
    Preparation,
    PreparationSummary,
    artefacts::{self, Eviction},
    stamp::{
        Fingerprint,
        Stamp,
        StampDecision,
        matching_stamp,
        missing_stamp_message,
        read_stamp,
        render,
        stamp_path,
        unreadable_stamp_error,
        write_stamp,
    },
};

/// What the build-capable gate did, as a value rather than only a side effect.
///
/// A caller that needs to know whether it paid for a build — to report it, to
/// record it, or to assert it in a test — would otherwise have to infer it from
/// timing or from the stamp file. Naming the two outcomes makes the difference
/// part of the contract instead: [`Self::Reused`] took no lock and ran no
/// closure, [`Self::Prepared`] did both.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PreparationOutcome {
    /// The stamp matched and every artefact it vouches for was present.
    ///
    /// No build ran, no lock was taken and no stamp was written.
    Reused,
    /// The family was prepared, and its stamp was written after the build
    /// succeeded.
    Prepared,
}

impl PreparationOutcome {
    /// Whether this outcome means a build ran.
    #[must_use]
    pub fn built(self) -> bool { matches!(self, Self::Prepared) }
}

/// Ensure the family's artefacts are prepared, running `prepare` if needed.
///
/// **This entry point may build.** Its name and its [`PreparationOutcome`]
/// say so: a caller reaching here must accept that, under plain `cargo test`,
/// the closure may compile a dependency graph. A caller asking whether the tree
/// is reusable *right now* wants [`admit`] instead, which performs no build.
///
/// A stamp that matches the current fingerprint returns
/// [`PreparationOutcome::Reused`] immediately. A missing or stale stamp under
/// `cargo nextest` returns a
/// [`io::ErrorKind::NotFound`] error naming the preparation command instead of
/// preparing inside a timed test. Under plain `cargo test` the closure runs
/// under a process-wide lock, and the stamp is written only after it succeeds.
///
/// The runner decision is a parameter rather than a read of the process
/// environment. Reading `NEXTEST_RUN_ID` here would make the gate's behaviour a
/// property of the ambient process that no call site states and no test can
/// vary without mutating the environment — which would race every other test in
/// the same binary. Every production call site resolves it once with
/// [`super::super::running_under_nextest`] and passes the result.
///
/// # Errors
///
/// The two runners do not report the same set, because they make opposite
/// decisions about the same evidence.
///
/// Under `cargo nextest` this returns the probe's error, the I/O error when the
/// stamp cannot be read, the I/O error when an artefact directory cannot be
/// inspected while the refusal is being explained, and the
/// [`io::ErrorKind::NotFound`] refusal for a missing or stale stamp. It never
/// runs `prepare`, so it never returns the closure's error.
///
/// Under plain `cargo test` a read failure is deliberately *not* reported: it
/// is logged and the family is prepared again, because a corrupt stamp would
/// otherwise have no remedy but deleting the tree by hand. What this returns
/// there is the probe's error, the closure's error when preparation fails, the
/// I/O error when the prepared artefacts cannot be checked, and the I/O error
/// when the stamp cannot be written.
pub fn ensure_prepared_building<F>(
    preparation: &Preparation,
    under_nextest: bool,
    prepare: F,
) -> io::Result<PreparationOutcome>
where
    F: FnOnce() -> io::Result<PreparationSummary>,
{
    ensure_prepared_building_reported(
        preparation,
        under_nextest,
        instruments_of(preparation),
        prepare,
    )
}

/// [`ensure_prepared_building`] with the diagnostic reporter supplied.
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
/// As [`ensure_prepared_building`], plus the probe's own error.
#[doc(hidden)]
pub fn ensure_prepared_building_reported<P, R, F>(
    preparation: &Preparation,
    under_nextest: bool,
    instruments: Instruments<P, R>,
    prepare: F,
) -> io::Result<PreparationOutcome>
where
    P: FnOnce() -> io::Result<String>,
    R: FnMut(&str),
    F: FnOnce() -> io::Result<PreparationSummary>,
{
    let Instruments { probe, mut report } = instruments;
    let reading = read_stamp(preparation, probe)?;
    let stamp = stamp_path(preparation);
    // An unreadable stamp is neither a match nor an absence, and the two
    // runners reach opposite conclusions from it. Under `nextest` it is
    // fail-fast evidence — the tree is present but cannot be vouched for, which
    // a "missing or stale" message would misreport. Under `cargo test` the
    // caller prepares again, so the read failure only costs a rebuild.
    let matching = match reading.decision {
        StampDecision::Matching(found) => Some(found),
        StampDecision::Missing | StampDecision::Stale => None,
        StampDecision::Unreadable(error) if !under_nextest => {
            report(&format!("stamp {} unreadable ({error})", stamp.display()));
            None
        }
        StampDecision::Unreadable(error) => {
            return Err(unreadable_stamp_error(&stamp, &error));
        }
    };
    // Both halves are required: a stamp vouches for the inputs, and the tree
    // must still hold the units it recorded.
    if matching.is_some() && artefacts::all_present(preparation, matching.as_ref())? {
        report(&format!(
            "family={} is prepared ({})",
            preparation.family,
            stamp.display()
        ));
        return Ok(PreparationOutcome::Reused);
    }

    if under_nextest {
        // The inspection happens here, once the decision to refuse is already
        // made, and its result is passed to a builder that performs no I/O of
        // its own. An inspection that fails is reported as itself rather than
        // folded into the refusal: a message assembled from a partial read
        // would name directories it never established anything about.
        let evicted: Vec<String> = artefacts::evicted(preparation, matching.as_ref())?
            .iter()
            .map(Eviction::describe)
            .collect();
        return Err(io::Error::new(
            io::ErrorKind::NotFound,
            missing_stamp_message(preparation, &stamp, &evicted),
        ));
    }

    // The fingerprint composed above is handed to `prepare_locked`, whose
    // second stamp read under the lock is what catches another thread's
    // preparation while this one waited.
    prepare_locked(
        preparation,
        &Target {
            stamp: &stamp,
            fingerprint: &reading.fingerprint,
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
pub(super) struct Target<'a> {
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
/// **This function may build**: it runs `prepare` under the lock when the
/// stamp is missing, stale or evicted. Its name says so, which is why the
/// read-only question lives in [`admit`] instead.
///
/// The toolchain identity and the fingerprint composed from it are resolved
/// before the lock is taken, so a probe that spawns `cargo -vV` does not extend
/// the time another thread waits to find out whether it needs to build.
///
/// `report` receives every diagnostic this function emits, including the
/// post-lock read failure explained below.
pub(super) fn prepare_locked<R, F>(
    preparation: &Preparation,
    target: &Target<'_>,
    mut report: R,
    prepare: F,
) -> io::Result<PreparationOutcome>
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
    // path is only taken after `ensure_prepared_building_reported` chose to
    // re-prepare — so refusing here would leave a corrupt stamp with no remedy
    // but deleting the tree by hand. What was wrong before was the silence:
    // `.ok()` made an unreadable stamp indistinguishable from a warm one that
    // had merely missed, and the rebuild happened with no explanation. The
    // wording names the second read so this line is not mistaken for the
    // pre-lock one.
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
        return Ok(PreparationOutcome::Reused);
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
    Ok(PreparationOutcome::Prepared)
}
