//! Every decision a scenario run makes, and no I/O.
//!
//! The split this file exists for: the two drivers (`drive_sync` and
//! `drive_async`) differ only in how they *await* a step, so anything they
//! would otherwise both decide would exist twice and could drift. The stop
//! decision is the one that matters — INV-1 hangs on it — so it lives here,
//! once, as a total function of a step's error.
//!
//! Nothing here takes a `StepContext`, touches the step registry, or awaits.
//! That is deliberate rather than incidental: it is what makes the whole of
//! the decision logic checkable with hand-built values, and what stops a
//! future change from smuggling I/O into the layer the drivers both trust.
//!
//! The name is broader than "skip" on purpose: [`SkipPolicy`] is the one rule
//! here that the *outcome* layer must agree with, but [`classify`] and
//! [`assemble`] are the drivers' shared control flow and have no counterpart on
//! the record side.

use crate::runner::{
    ScenarioFailure,
    ScenarioOutcome,
    ScenarioSkip,
    ScenarioStatus,
    SkipPolicyRecord,
    SkipRecord,
    ValueFate,
    outcome::StepOutcome,
    source::SourceLocation,
};

/// A step result with its returned value already absorbed.
///
/// The two fields are mutually exclusive by construction: `fate` is `Some`
/// only when the step returned a value, and a step that returned a value did
/// not fail. They are separate fields rather than one sum because merging them
/// would mean deciding between "ran" and "failed" here, and that
/// discrimination is [`classify`]'s job.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct Absorbed {
    /// What became of the step's returned value, when it returned one.
    pub(crate) fate: Option<ValueFate>,
    /// The error the step ended with, when it ended with one.
    pub(crate) error: Option<crate::ExecutionError>,
}

/// Absorb a step result: insert any returned value through `insert`, and keep
/// the insertion's fate alongside the error, if any.
///
/// The insertion is a parameter rather than a `StepContext` because this
/// module is context-free by contract (LEM-1); the closure is how the driver
/// supplies the context without the engine naming it. Making insertion
/// required is also what makes "insertion happens before classification"
/// structural: there is no path to [`classify`] that skipped it.
pub(crate) fn absorb(
    result: Result<Option<Box<dyn std::any::Any>>, crate::ExecutionError>,
    insert: impl FnOnce(Box<dyn std::any::Any>) -> ValueFate,
) -> Absorbed {
    match result {
        Ok(value) => Absorbed {
            fate: value.map(insert),
            error: None,
        },
        Err(error) => Absorbed {
            fate: None,
            error: Some(error),
        },
    }
}

/// What one step result means for the run.
///
/// Three variants, and the driver's body is a single `match` on this value —
/// which is where "no `if` on a step result" is actually enforced. `classify`
/// itself may branch freely, because it is the designated home for the policy.
///
/// The terminal variants are separate because a *permitted* skip carries no
/// failure: an outcome stores `skip` and `failure` independently, and
/// [`ScenarioFailure::ForcedSkip`] is derived at fold time rather than stored.
/// A single `Stop(ScenarioFailure)` could not represent it.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) enum StepDecision {
    /// The step ran. Any returned value was inserted by [`absorb`].
    Continue,
    /// The step requested a skip; the run stops and the outcome will skip.
    Skip {
        /// The reason the step supplied, when it supplied one.
        message: Option<String>,
    },
    /// The step failed; the error is the terminal failure, carried verbatim.
    Fail(crate::ExecutionError),
}

/// The event that stopped a run, named by the invocation it happened at.
///
/// Built by the driver from the same values it recorded as a `StepOutcome`, so
/// [`assemble`] never has to scan the record for the terminal entry. Carrying
/// the plan-side identity here is what keeps `status: Skipped` and
/// `skip: Some(_)` inseparable: one match arm in [`assemble`] produces both.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) enum Terminal {
    /// The run stopped because a step requested a skip.
    Skip {
        /// Zero-based index of the skipping invocation.
        index: usize,
        /// The reason the step supplied, when it supplied one.
        message: Option<String>,
        /// Where the skipping invocation was written, from the plan.
        source: Option<SourceLocation>,
    },
    /// The run stopped because a step failed.
    Fail {
        /// Zero-based index of the failing invocation.
        index: usize,
        /// The error that ended the run.
        error: crate::ExecutionError,
    },
}

/// Classify one step's error.
///
/// `None` is a step that ran; `Some` is a step that did not. The single
/// discrimination is [`ExecutionError::is_skip`], because a skip is control
/// flow and everything else is a failure. Failure *labels* are deliberately
/// not computed here: [`FailureKind`](crate::runner::FailureKind) is a
/// projection of the error that a reporter can apply later, and a decision
/// carrying one would duplicate it.
#[must_use]
pub(crate) fn classify(error: Option<crate::ExecutionError>) -> StepDecision {
    match error {
        None => StepDecision::Continue,
        Some(error) if error.is_skip() => StepDecision::Skip {
            message: error.skip_message().map(str::to_owned),
        },
        Some(error) => StepDecision::Fail(error),
    }
}

/// The resolved skip policy for one run, as the record needs it.
///
/// Computed once per run, so that a run cannot observe the process-global
/// configuration changing part-way through it. The two halves are read at
/// different times and that is deliberate: `ScenarioScope::new` reads
/// `config::fail_on_skipped()` and stores it, and the driver calls
/// [`resolve`](Self::resolve) once the plan's own flag is also in hand, because
/// the plan is not available to the scope.
///
/// The fields are private and the two questions are asked as one because
/// [`ScenarioSkip::new`] must record *both*, and they are not independent:
/// `forced_failure` is exactly `!allow_skipped && fail_on_skipped`. Asking them
/// as one leaves the caller no way to hand over a pair the record's own
/// invariant would then have to reject.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) struct SkipPolicy {
    /// Whether this run permits a skip without failing the suite.
    ///
    /// The *effective* flag: the plan's own value, or the explicit per-run
    /// override, already folded with `fail_on_skipped`. The `||
    /// !fail_on_skipped` term in [`resolve`](Self::resolve) is what makes it
    /// effective, so the record's invariant holds of the recorded value and not
    /// merely of the policy that produced it.
    allow_skipped: bool,
    /// The resolved `fail_on_skipped` for this run.
    fail_on_skipped: bool,
}

impl SkipPolicy {
    /// Resolve a policy from a plan's own flag and the process-global setting.
    #[must_use]
    pub(crate) const fn resolve(plan_allows_skipping: bool, fail_on_skipped: bool) -> Self {
        Self {
            allow_skipped: plan_allows_skipping || !fail_on_skipped,
            fail_on_skipped,
        }
    }

    /// The permission and the forced-failure flag, for `ScenarioSkip`.
    ///
    /// Returned as a pair rather than as two accessors so the record cannot be
    /// built with one of them and then the other read from a different policy:
    /// the two are a property of one resolved run, and a caller that asked for
    /// them separately could interleave another resolve between the two.
    #[must_use]
    pub(crate) const fn record(self) -> (bool, bool) {
        (self.allow_skipped(), self.forced_failure())
    }

    /// Whether this run permits a skip without failing the suite.
    ///
    /// The *effective* flag. [`Self::resolve`]'s `|| !fail_on_skipped` term is
    /// what makes it effective, so the record's own invariant holds of the
    /// value this returns and not merely of the policy that produced it.
    #[must_use]
    pub(crate) const fn allow_skipped(self) -> bool { self.allow_skipped }

    /// The resolved `fail_on_skipped` for this run.
    ///
    /// Read for the tracing span, not for the record: the record carries the
    /// two *derived* values, and `fail_on_skipped` alone is not one of them.
    #[must_use]
    pub(crate) const fn fail_on_skipped(self) -> bool { self.fail_on_skipped }

    /// Whether a skip under this policy must fail the suite.
    ///
    /// Exactly `!allow_skipped && fail_on_skipped`, resolved once per run.
    #[must_use]
    pub(crate) const fn forced_failure(self) -> bool { !self.allow_skipped && self.fail_on_skipped }
}

/// Assemble the terminal outcome from the recorded details and the terminal.
///
/// Reports [`ScenarioStatus::Passed`] for an empty plan, because running
/// nothing is not itself an error. The fold is deliberately stricter and
/// rejects it; that rule is not applied here.
///
/// The `#[must_use]` is deliberately absent: `ScenarioOutcome` already carries
/// it on the type, which is D13's whole point, so repeating it here would only
/// trigger `clippy::double_must_use`.
pub(crate) fn assemble(
    details: Vec<StepOutcome>,
    terminal: Option<Terminal>,
    policy: SkipPolicy,
) -> ScenarioOutcome {
    match terminal {
        None => ScenarioOutcome::new(ScenarioStatus::Passed, details, None, None),
        Some(Terminal::Skip {
            index,
            message,
            source,
        }) => {
            let (allow_skipped, forced_failure) = policy.record();
            let skip = ScenarioSkip::new(
                index,
                SkipRecord { message, source },
                SkipPolicyRecord {
                    allow_skipped,
                    forced_failure,
                },
            );
            ScenarioOutcome::new(ScenarioStatus::Skipped, details, Some(skip), None)
        }
        Some(Terminal::Fail { index, error }) => ScenarioOutcome::new(
            ScenarioStatus::Failed,
            details,
            None,
            Some(ScenarioFailure::Step { index, error }),
        ),
    }
}
