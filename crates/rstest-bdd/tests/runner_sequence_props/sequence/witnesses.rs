//! The accumulator that turns "every case passed" into "the domain was reached".
//!
//! Split out of `sequence/mod.rs` to keep both files inside the repository's
//! 400-line cap. The vocabulary and the harness stay in the parent; this file
//! holds the non-vacuity machinery, which is the part of the suite that has to
//! be read to be trusted.
//!
//! # Why the assertions run after the cases, not inside them
//!
//! Each of these invariants is satisfied by a generator that produces no
//! interesting case at all: a run that never stops early satisfies INV-1
//! trivially, and a run with no value-returning step satisfies INV-3 and INV-12
//! trivially. Asserting only inside the cases would let a generator regression
//! turn the suite *green*. So each case folds what it saw into a [`Witnesses`]
//! and the property asserts afterwards that every class its invariant names was
//! reached.

use rstest_bdd::runner::{FailureKind, ScenarioStatus, ValueFate};

use super::{MAX_STEPS, Run, SENTINEL, Step};

/// The classes each property's non-vacuity claim requires.
///
/// Folded across every case and asserted *after* the run, because each of these
/// invariants is satisfied by a generator that produces no interesting case at
/// all: a run that never stops early satisfies INV-1 trivially, and a run with
/// no value-returning step satisfies INV-3 and INV-12 trivially. Asserting only
/// inside the cases would let a generator regression turn the suite *green*.
// Four of these record "has this ever been observed", and each is asserted
// separately: the two visibility clauses are independent halves of INV-3, and
// the two domain clauses are independent extremes of the generator. Collapsing
// them into a state would imply the observations were exclusive when a case
// routinely sets several at once.
#[expect(
    clippy::struct_excessive_bools,
    reason = "four independent existential witnesses, not a state machine"
)]
#[derive(Debug, Default)]
pub(crate) struct Witnesses {
    /// The status every case's run ended with.
    statuses: Vec<ScenarioStatus>,
    /// The classification of every record that failed, wherever one did.
    kinds: Vec<FailureKind>,
    /// The fate of every value-returning invocation, wherever one was recorded.
    fates: Vec<ValueFate>,
    /// Whether any case placed an observer before its first producer.
    observer_before_producer: bool,
    /// Whether any case observed a producer's value.
    observer_saw_producer: bool,
    /// Whether any case generated a plan whose run stopped before its end.
    stopped_early: bool,
    /// Whether any case generated an empty plan.
    empty_plan: bool,
}

impl Witnesses {
    /// Fold one case in.
    pub(crate) fn record(&mut self, plan: &[Step], run: &Run) {
        self.statuses.push(run.outcome.status());
        self.kinds.extend(run.failure_kinds());
        self.empty_plan |= plan.is_empty();
        self.stopped_early |= run
            .terminal_index()
            .is_some_and(|terminal| terminal + 1 < plan.len());
        self.fates
            .extend(run.fates(plan).into_iter().filter_map(|(_, fate)| fate));

        // The indices of the invocations that hand a value back. INV-3's
        // negative clause is about these: an observer must not see a value
        // whose producer has not run yet.
        let producers: Vec<usize> = plan
            .iter()
            .enumerate()
            .filter(|(_, step)| step.kind.returns_a_value())
            .map(|(index, _)| index)
            .collect();

        for reading in &run.readings {
            match reading.value.filter(|&seen| seen != SENTINEL) {
                // A value was visible, and it carries the identity of the
                // invocation that produced it. So the relation is read from
                // `seen` rather than from where the producers sit in the plan.
                Some(seen) => self.observer_saw_producer |= seen < reading.observer,
                // The observer read the fixture's own value, or found no name
                // at all. That is INV-3's negative clause *holding* — but only
                // a case with a producer it could have seen makes it evidence.
                //
                // Two conditions make the witness real rather than incidental.
                // The producer must sit after the observer, so the value's
                // producer had not run when the observer read; and the producer
                // must have run eventually, or the value never existed at all
                // and the observer seeing nothing would be true of any driver,
                // including one that hands every observer a future value.
                // Together they are the case a correct driver passes and an
                // eager one fails.
                //
                // Reading this from `seen` instead — requiring the observer to
                // have read a *later* producer's value — would make the flag
                // unsatisfiable rather than merely rare, since a value can only
                // travel forwards. That is the direction the invariant forbids,
                // so no correct run can produce it.
                None => {
                    self.observer_before_producer |= producers.iter().any(|&producer| {
                        producer > reading.observer && run.executed.contains(&producer)
                    });
                }
            }
        }
    }

    /// Assert every terminal status was reached, and every classification the
    /// plan's INV-1 non-vacuity list names.
    ///
    /// The statuses and the classifications are asserted separately because two
    /// kinds share a status: `HandlerError` and `Panic` both end a run
    /// `Failed`, so a generator reaching only one of them would satisfy a
    /// status-only check while leaving half the list unreached.
    pub(crate) fn assert_complete(&self) {
        for expected in [
            ScenarioStatus::Passed,
            ScenarioStatus::Skipped,
            ScenarioStatus::Failed,
        ] {
            assert!(
                self.statuses.contains(&expected),
                "the generator never produced a run ending {expected:?}; it produced {:?}. A \
                 non-vacuity claim about termination needs every terminal status reachable.",
                self.statuses
            );
        }
        for expected in [
            FailureKind::Assertion,
            FailureKind::Undefined,
            FailureKind::MissingFixture,
            FailureKind::Panic,
        ] {
            assert!(
                self.kinds.contains(&expected),
                "the generator never produced a failure classified {expected:?}; it produced {:?}",
                self.kinds
            );
        }
        assert!(
            self.empty_plan,
            "the domain includes the empty plan (length 0 to {MAX_STEPS})",
        );
        assert!(
            self.stopped_early,
            "no case's run stopped before its last invocation, so INV-1 and INV-2's \
             bypassed-trailing-invocation clauses were never exercised",
        );
    }

    /// Assert every fate was reached, including the one that is silent.
    pub(crate) fn assert_fates_complete(&self) {
        for expected in [
            ValueFate::Inserted,
            ValueFate::NoMatch,
            ValueFate::AmbiguousIgnored,
        ] {
            assert!(
                self.fates.contains(&expected),
                "the generator never recorded a {expected:?} fate; it recorded {:?}. `NoMatch` \
                 emits no warning anywhere in the runtime, so a generator that stopped reaching \
                 it would leave INV-12 discharging nothing while the suite stayed green.",
                self.fates
            );
        }
    }

    /// How many cases were classified, and how many recorded each class.
    ///
    /// Exposed as counts rather than as the fields themselves so that a caller
    /// outside this module can ask "was this class reached often enough to be
    /// a property rather than a lucky draw?" — a question the completeness
    /// assertions deliberately do not ask, because a class reached once
    /// satisfies "the generator reaches it" while giving very weak evidence
    /// that it will be reached again. The counts are reported grouped by
    /// class, so a caller never has to know the iteration order.
    pub(crate) fn tally(&self) -> Tally {
        Tally {
            cases: self.statuses.len(),
            statuses: class_counts(&self.statuses),
            fates: class_counts(&self.fates),
        }
    }

    /// Assert the observer domain reached both sides of a producer.
    pub(crate) fn assert_visibility_complete(&self) {
        assert!(
            self.observer_before_producer,
            "no case placed an observer before the first producer, so INV-3's \"and to no \
             invocation `j <= i`\" clause was never exercised. The sentinel is what makes it \
             falsifiable, and this assertion is the evidence it was tried.",
        );
        assert!(
            self.observer_saw_producer,
            "no observer ever read a producer's value, so the positive half of INV-3 was never \
             exercised and every case would pass against a driver that inserted nothing at all.",
        );
    }
}

/// Count how often each class occurred, dropping the ones that never did.
///
/// A free function rather than a method: it reads no field, and the two counts
/// it serves are over different fields, so a `&self` here would suggest a
/// receiver it does not use.
fn class_counts<T: Copy + Eq>(seen: &[T]) -> Vec<(T, usize)> {
    let mut counts: Vec<(T, usize)> = Vec::new();
    for &value in seen {
        match counts.iter_mut().find(|(class, _)| *class == value) {
            Some((_, count)) => *count += 1,
            None => counts.push((value, 1)),
        }
    }
    counts
}

/// How often each class was reached across every case of one property.
///
/// Returned by [`Witnesses::tally`]. The counts are the evidence a class is a
/// property of the strategy rather than a lucky draw; the completeness
/// assertions cover the weaker claim that it occurred at all.
#[derive(Debug, Default)]
pub(crate) struct Tally {
    /// How many cases were classified.
    pub(crate) cases: usize,
    /// How often each terminal status was reached, and how often each
    /// classification was recorded.
    pub(crate) statuses: Vec<(ScenarioStatus, usize)>,
    /// How often each fate was recorded.
    pub(crate) fates: Vec<(ValueFate, usize)>,
}

impl Tally {
    /// How often `status` was the run's outcome.
    pub(crate) fn status(&self, status: ScenarioStatus) -> usize {
        self.statuses
            .iter()
            .find(|(class, _)| *class == status)
            .map_or(0, |(_, count)| *count)
    }

    /// How often `fate` was recorded.
    pub(crate) fn fate(&self, fate: ValueFate) -> usize {
        self.fates
            .iter()
            .find(|(class, _)| *class == fate)
            .map_or(0, |(_, count)| *count)
    }
}
