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

/// The terminal skip, retained even when a later cleanup failure upgrades the
/// overall status to [`Failed`](ScenarioStatus::Failed).
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
    /// The reason the step supplied, when it supplied one.
    message: Option<String>,
    /// Where the skipping step was written, when the plan recorded it.
    source: Option<SourceLocation>,
    /// Whether the scenario explicitly permits skipping.
    allow_skipped: bool,
    /// `!allow_skipped && fail_on_skipped`, resolved once per run.
    forced_failure: bool,
}

impl ScenarioSkip {
    /// Construct a skip record.
    ///
    /// The production caller is the runner's engine, which lands in EP-M2. In a
    /// `cfg(test)` build the expectation is absent, so the unit tests that build
    /// outcomes synthetically do not leave it unfulfilled; once EP-M2 constructs
    /// skips it becomes unfulfilled in a normal build too, which fails the lint
    /// until the attribute is deleted rather than lingering unnoticed.
    pub(crate) fn new(
        at: usize,
        message: Option<String>,
        source: Option<SourceLocation>,
        allow_skipped: bool,
        forced_failure: bool,
    ) -> Self {
        Self {
            at,
            message,
            source,
            allow_skipped,
            forced_failure,
        }
    }

    /// Return the zero-based index of the step that requested the skip.
    #[must_use]
    pub const fn at(&self) -> usize { self.at }

    /// Borrow the reason the step supplied, when it supplied one.
    #[must_use]
    pub fn message(&self) -> Option<&str> { self.message.as_deref() }

    /// Borrow where the skipping step was written, when the plan recorded it.
    #[must_use]
    pub const fn source(&self) -> Option<&SourceLocation> { self.source.as_ref() }

    /// Whether the scenario explicitly permitted skipping.
    #[must_use]
    pub const fn allow_skipped(&self) -> bool { self.allow_skipped }

    /// Whether skip policy converts this skip into a suite failure.
    ///
    /// Exactly `!allow_skipped && fail_on_skipped`, resolved once per run.
    #[must_use]
    pub const fn forced_failure(&self) -> bool { self.forced_failure }
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
