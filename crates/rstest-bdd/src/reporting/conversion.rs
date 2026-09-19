//! D5: the one-way conversion from a runner outcome to a reporting record.
//!
//! The arrow points this way and no other way. [`crate::runner`] is the
//! canonical outcome model and knows nothing about reporters; this module is
//! the only place the two meet, and it is `#[cfg(test)]` so that no conversion
//! function ships before 13.2.1 needs one. Everything here is a test artefact.
//!
//! # What this proves, and what it cannot
//!
//! D5 asked for the conversion now because attempting it is the cheapest proof
//! that the two models are reconcilable, and because the places they are *not*
//! reconcilable fall out of the attempt. That is what [`Gap`] is: a named work
//! item for 13.2.1 rather than a silent loss. The gaps are this module's
//! finding, not a defect in it.
//!
//! What it cannot be is end-to-end. A `#[cfg(test)]` module is compiled only
//! for this crate's own unit tests, so the conversion is smoke-tested against a
//! hand-built outcome and never against a live run. D5 records that narrowing
//! rather than closing it.
//!
//! # Why the conversion takes a plan as well as an outcome
//!
//! A [`ScenarioOutcome`] carries no scenario identity: no name, no path, no
//! tags, no declared line. All four are plan-side. The signature is therefore
//! itself the record of one of D5's obligations — `reporting` needs a name and
//! tags the outcome does not carry, and cannot be given them without widening
//! the outcome surface that ADR-018 keeps reporter-free.

use crate::{
    reporting::{ScenarioMetadata, ScenarioRecord, ScenarioStatus, SkippedScenario},
    runner::{ScenarioOutcome, ScenarioPlan, ScenarioStatus as RunnerStatus},
};

mod tests;

/// A place where a plan and its outcome have no `reporting` representation.
///
/// Three variants, each a 13.2.1 work item that would otherwise have been
/// discovered during the migration rather than before it. Naming them is the
/// product of D5's "write the conversion now" instruction.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Gap {
    /// The run failed, and `reporting::ScenarioStatus` has no failure case: it
    /// can say `Passed` or `Skipped` and nothing else. 13.2.1 must add one.
    ///
    /// This is the gap D5 exists to surface, and it is why the conversion
    /// cannot be a `From` impl.
    Failure,
    /// The plan recorded no scenario line, and [`ScenarioMetadata`] requires
    /// one. A dynamic frontend is free to omit the line, so this is reachable
    /// rather than theoretical.
    MissingLine,
    /// The run reported `Skipped` without a skip record.
    ///
    /// The engine cannot produce this: `assemble` writes the status and the
    /// skip record together, from one match arm. It is handled rather than
    /// `unreachable!()`d because a conversion that panics on its input is a
    /// worse neighbour than one that reports, and because `unreachable!()`
    /// would assert something this module cannot check. The test module builds
    /// the state by hand so the branch is observed to fire, which is what makes
    /// it a guard rather than decoration.
    MissingSkipRecord,
}

/// Convert a plan and its outcome into the record the reporting pipeline takes.
///
/// Returns a [`Gap`] rather than a partial record when the pair has no
/// representation: a `ScenarioRecord` that dropped the reason it was built
/// would report a passing skip for a failed run, which is the canonical false
/// green.
///
/// # Errors
///
/// Returns:
///
/// - [`Gap::MissingLine`] when `plan` recorded no scenario line;
/// - [`Gap::Failure`] when the run failed;
/// - [`Gap::MissingSkipRecord`] when the run reported a skip with no record of one.
fn record_from(plan: &ScenarioPlan, outcome: &ScenarioOutcome) -> Result<ScenarioRecord, Gap> {
    let line = plan.source_line().ok_or(Gap::MissingLine)?;
    let status = match outcome.status() {
        RunnerStatus::Passed => ScenarioStatus::Passed,
        RunnerStatus::Skipped => {
            let skip = outcome.skip().ok_or(Gap::MissingSkipRecord)?;
            ScenarioStatus::Skipped(SkippedScenario::new(
                skip.message().map(str::to_owned),
                skip.allow_skipped(),
                skip.forced_failure(),
            ))
        }
        RunnerStatus::Failed => return Err(Gap::Failure),
    };
    let metadata = ScenarioMetadata::new(
        plan.source(),
        plan.name(),
        line,
        plan.tags().map(str::to_owned).collect::<Vec<_>>(),
    );
    Ok(ScenarioRecord::from_metadata(metadata, status))
}

// The conversion's signature, written out, as a type-level assertion.
//
// The coercion fails to compile if either parameter or the return type stops
// being exactly these types — which is the check D5 asks for, and which a test
// that merely *called* the conversion would not make. Writing the signature a
// second time is the point: a change that widens the function cannot be
// absorbed by editing one line, because this line has to agree with it.
//
// It is an anonymous `const` because it exists only to be checked. A named
// `const` would be dead code by construction — nothing has a reason to read
// it — and `#[expect(dead_code)]` would be the wrong remedy, since the
// expectation is the assertion, not the silence.
//
// What it pins is the signature and the outer names it mentions:
// `ScenarioPlan`, `ScenarioOutcome`, `ScenarioRecord`, and `Gap`. It does not
// reach inside those types, so a frontend type nested in a field of one of
// them would still coerce. That residual hole is the same one
// `runner/tests/surface.rs` documents for its token scan, and it is left open
// deliberately: closing it needs a compiler pass over the crate's public API,
// which is a much larger instrument than a unit test.
const _: for<'p, 'o> fn(&'p ScenarioPlan, &'o ScenarioOutcome) -> Result<ScenarioRecord, Gap> =
    record_from;
