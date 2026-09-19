//! The structured terminal outcome of a scenario run.
//!
//! [`ScenarioOutcome`] is opaque: every payload sits behind an accessor and
//! [`ScenarioStatus`] is fieldless, so the representation can move freely.
//! Adding a field to a public struct variant of a `#[non_exhaustive]` enum
//! still breaks every downstream `match` that does not write `..`, which is
//! what this shape exists to avoid.

mod failure;
mod step;

pub use failure::{FailureKind, FailureSite, ScenarioFailure};
#[cfg(test)]
pub(crate) use step::test_invocation;
pub use step::{StepOutcome, StepStatus, ValueFate};

use crate::runner::source::SourceLocation;

/// The terminal status of a scenario run.
///
/// Fieldless and `#[non_exhaustive]`, so it can gain cases without breaking a
/// caller's `match`.
///
/// # Examples
///
/// ```
/// use rstest_bdd::runner::ScenarioStatus;
///
/// assert_ne!(ScenarioStatus::Passed, ScenarioStatus::Failed);
/// ```
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
#[non_exhaustive]
pub enum ScenarioStatus {
    /// Every planned step ran and none failed.
    ///
    /// An empty plan also reports this, because running nothing is not itself
    /// an error. [`ScenarioOutcome::into_harness_result`] is stricter and
    /// rejects an empty plan; use that fold, not this status, to decide whether
    /// a run should fail a suite.
    Passed,
    /// A step requested a skip.
    Skipped,
    /// A step failed.
    ///
    /// Only a step failure sets this. An empty plan is *not* a failure here —
    /// see [`Passed`](Self::Passed) — so a caller that treats this status as
    /// "the run may proceed" is not misled by a malformed document that parsed
    /// to no steps, provided it consults
    /// [`ScenarioOutcome::into_harness_result`] for the success test.
    Failed,
}

/// The resolved skip policy, as a record rather than as a rule.
///
/// Both fields are the *resolved* values, so a reader asking "would this skip
/// have failed the suite?" gets the run's own answer rather than a rule it
/// would have to re-evaluate against a policy it no longer has.
///
/// # Examples
///
/// ```
/// use rstest_bdd::runner::SkipPolicyRecord;
/// // Records are produced by the runner.
/// # let _ = std::marker::PhantomData::<SkipPolicyRecord>;
/// ```
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct SkipPolicyRecord {
    /// The *effective* permission to skip, which is not the plan's own flag.
    ///
    /// A run grants permission when the plan asks for it **or** when
    /// `fail_on_skipped` is off, so this holds
    /// `plan.allow_skipped() || !fail_on_skipped` rather than the plan's flag
    /// alone. Recording the effective value is what makes
    /// `forced_failure == !allow_skipped && fail_on_skipped` hold of the record
    /// itself and not merely of the policy that built it.
    pub allow_skipped: bool,
    /// `!allow_skipped && fail_on_skipped`, resolved once per run.
    pub forced_failure: bool,
}

/// Why a run stopped early, in the words the invocation gave.
///
/// The pair a skip record's *message* half is built from, which is why the two
/// travel together: a caller storing one and reading the other from elsewhere
/// could pair a message with a location that did not produce it. The policy
/// half is [`SkipPolicyRecord`].
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SkipRecord {
    /// The reason the step supplied, when it supplied one.
    pub message: Option<String>,
    /// Where the skipping step was written, when the plan recorded it.
    pub source: Option<SourceLocation>,
}

/// The terminal skip, retained so a caller can see *why* the run stopped
/// early.
///
/// A skip is not itself a failure: whether the suite treats it as one is
/// [`forced_failure`](Self::forced_failure)'s job, and
/// [`ScenarioOutcome::into_harness_result`] is the one place that fold happens.
///
/// Nothing later in the run can upgrade the status to
/// [`Failed`](ScenarioStatus::Failed). A value destructor that panics during
/// cleanup is caught and logged by the scope's cleanup guard and does not
/// reach the outcome, because `ScenarioOutcome` carries exactly one failure
/// channel — the `cleanup_error` field that would have carried it was dropped
/// with the hooks under D2 option (ii). See
/// [`ScenarioFailure`] for the channels that do exist.
///
/// # Examples
///
/// ```
/// use rstest_bdd::runner::ScenarioSkip;
/// // Skip records are produced by the runner.
/// # let _ = std::marker::PhantomData::<ScenarioSkip>;
/// ```
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ScenarioSkip {
    /// Zero-based index of the step that requested the skip.
    at: usize,
    /// Why it stopped, in the invocation's own words.
    record: SkipRecord,
    /// The policy the run had resolved when it did.
    policy: SkipPolicyRecord,
}

impl ScenarioSkip {
    /// Construct a skip record.
    ///
    /// The caller is the runner's engine, which builds one whenever an
    /// invocation asks to be skipped.
    ///
    /// `#[must_use]`: a skip built and dropped would leave the invocation that
    /// requested it recorded as skipped with no reason attached, which is the
    /// silently-green shape [`ScenarioOutcome`] already guards against.
    #[must_use]
    pub fn new(at: usize, record: SkipRecord, policy: SkipPolicyRecord) -> Self {
        Self { at, record, policy }
    }

    /// Return the zero-based index of the step that requested the skip.
    #[must_use]
    pub const fn at(&self) -> usize { self.at }

    /// Borrow the reason the step supplied, when it supplied one.
    #[must_use]
    pub fn message(&self) -> Option<&str> { self.record.message.as_deref() }

    /// Borrow where the skipping step was written, when the plan recorded it.
    #[must_use]
    pub const fn source(&self) -> Option<&SourceLocation> { self.record.source.as_ref() }

    /// Whether this run permitted skipping, having resolved the policy.
    ///
    /// This is the *effective* permission, so it is `true` either because the
    /// plan asked for it or because `fail_on_skipped` was off. It is therefore
    /// not a readback of the plan's own `allow_skipped` flag, and a caller
    /// asking whether the *plan* permits skipping must consult the plan.
    #[must_use]
    pub const fn allow_skipped(&self) -> bool { self.policy.allow_skipped }

    /// Whether skip policy converts this skip into a suite failure.
    ///
    /// Exactly `!allow_skipped && fail_on_skipped`, resolved once per run.
    #[must_use]
    pub const fn forced_failure(&self) -> bool { self.policy.forced_failure }
}

/// The complete terminal outcome of one scenario run.
///
/// A dropped outcome is a silently green scenario, which is why
/// [`into_harness_result`](Self::into_harness_result) — and not
/// [`is_passed`](Self::is_passed) — is the sanctioned success test.
#[must_use = "a dropped outcome is a silently green scenario; see into_harness_result"]
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ScenarioOutcome {
    /// What the run ended as.
    status: ScenarioStatus,
    /// One entry per planned invocation, in plan order.
    steps: Vec<StepOutcome>,
    /// The terminal skip, when the run skipped.
    skip: Option<ScenarioSkip>,
    /// The failure that ended the run, when one did.
    failure: Option<ScenarioFailure>,
}

impl ScenarioOutcome {
    /// Assemble an outcome from its parts.
    ///
    /// The production caller is the runner's engine, which lands in EP-M2. See
    /// [`ScenarioSkip::new`] for why the expectation is `not(test)`-scoped.
    pub(crate) fn new(
        status: ScenarioStatus,
        steps: Vec<StepOutcome>,
        skip: Option<ScenarioSkip>,
        failure: Option<ScenarioFailure>,
    ) -> Self {
        Self {
            status,
            steps,
            skip,
            failure,
        }
    }

    /// Return the terminal status.
    ///
    /// # Examples
    ///
    /// ```
    /// use rstest_bdd::runner::{ScenarioOutcome, ScenarioStatus};
    /// // Outcomes are produced by the runner.
    /// # let _ = std::marker::PhantomData::<ScenarioOutcome>;
    /// # let _ = ScenarioStatus::Passed;
    /// ```
    #[must_use]
    pub const fn status(&self) -> ScenarioStatus { self.status }

    /// Borrow the per-invocation records, in plan order.
    #[must_use]
    pub fn steps(&self) -> &[StepOutcome] { &self.steps }

    /// Borrow the terminal skip, when the run skipped.
    #[must_use]
    pub const fn skip(&self) -> Option<&ScenarioSkip> { self.skip.as_ref() }

    /// Borrow the failure that ended the run, when one did.
    #[must_use]
    pub const fn failure(&self) -> Option<&ScenarioFailure> { self.failure.as_ref() }

    /// Borrow the source of the invocation that ended the run.
    ///
    /// # Examples
    ///
    /// ```
    /// use rstest_bdd::runner::ScenarioOutcome;
    /// // Outcomes are produced by the runner.
    /// # let _ = std::marker::PhantomData::<ScenarioOutcome>;
    /// ```
    #[must_use]
    pub fn terminal_source(&self) -> Option<&SourceLocation> {
        self.skip
            .as_ref()
            .and_then(ScenarioSkip::source)
            .or_else(|| match self.failure.as_ref() {
                Some(ScenarioFailure::Step { index, .. }) => {
                    self.steps.get(*index).and_then(StepOutcome::source)
                }
                _ => None,
            })
    }

    /// Whether the run was a clean pass.
    ///
    /// True only for [`ScenarioStatus::Passed`]. A skip always reports
    /// [`Skipped`](ScenarioStatus::Skipped), so this is `false` for a skip the
    /// suite is content to accept as well as for one policy forces to fail.
    /// Skip policy is deliberately not folded here: the one canonical
    /// [`into_harness_result`](Self::into_harness_result) owns it, and the two
    /// therefore disagree in the permitted-skip case — this returns `false`
    /// while the fold returns `Ok(())`.
    #[must_use]
    pub const fn is_passed(&self) -> bool { matches!(self.status, ScenarioStatus::Passed) }

    /// The one canonical success test.
    ///
    /// Folds `forced_failure` and the empty-plan rule, so a skip the caller is
    /// expected to treat as a failure — and a malformed document that parsed to
    /// no steps at all — cannot report success.
    ///
    /// # Errors
    ///
    /// Returns:
    ///
    /// - [`ScenarioFailure::Step`] when a step failed;
    /// - [`ScenarioFailure::ForcedSkip`] when the run skipped and `!allow_skipped &&
    ///   fail_on_skipped`;
    /// - [`ScenarioFailure::EmptyPlan`] when the plan contained no invocations.
    ///
    /// # Examples
    ///
    /// ```
    /// use rstest_bdd::runner::{ScenarioFailure, ScenarioOutcome};
    /// // Outcomes are produced by the runner.
    /// # let _ = std::marker::PhantomData::<ScenarioOutcome>;
    /// # let _ = ScenarioFailure::EmptyPlan;
    /// ```
    pub fn into_harness_result(self) -> Result<(), ScenarioFailure> {
        if let Some(failure) = self.failure {
            return Err(failure);
        }
        if self.steps.is_empty() {
            return Err(ScenarioFailure::EmptyPlan);
        }
        if let Some(skip) = self.skip.filter(ScenarioSkip::forced_failure) {
            return Err(ScenarioFailure::ForcedSkip(skip));
        }
        Ok(())
    }
}

impl std::fmt::Display for ScenarioOutcome {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self.status {
            ScenarioStatus::Passed => write!(f, "scenario passed ({} steps)", self.steps.len()),
            ScenarioStatus::Skipped => {
                let at = self.skip.as_ref().map_or(0, ScenarioSkip::at);
                match self.skip.as_ref().and_then(ScenarioSkip::message) {
                    Some(message) => write!(f, "scenario skipped at step {at}: {message}"),
                    None => write!(f, "scenario skipped at step {at}"),
                }
            }
            ScenarioStatus::Failed => match self.failure.as_ref() {
                Some(ScenarioFailure::Step { index, error }) => {
                    write!(f, "scenario failed at step {index}: {error}")
                }
                Some(ScenarioFailure::EmptyPlan) => {
                    write!(f, "scenario failed: the plan contained no steps")
                }
                Some(ScenarioFailure::ForcedSkip(skip)) => {
                    write!(f, "scenario failed: skipped at step {}", skip.at())
                }
                _ => write!(f, "scenario failed"),
            },
        }
    }
}
