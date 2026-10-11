//! Failure classification and terminal failure records.
//!
//! A [`ScenarioFailure`] carries a site and its payload together, so the two
//! cannot disagree. [`FailureKind`] is the stable projection a reporter can
//! branch on without matching [`ExecutionError`]'s variants directly.

use crate::{ExecutionError, StepError, runner::outcome::ScenarioSkip};

/// A stable projection of a step failure.
///
/// A reporter that must render failures chooses among these rather than
/// matching [`ExecutionError`]'s variants, which are free to grow. This is the
/// difference between extending the error representation and breaking every
/// reporter at once.
///
/// # Examples
///
/// ```
/// use rstest_bdd::runner::FailureKind;
///
/// let kind = FailureKind::of(&rstest_bdd::execution::ExecutionError::StepNotFound {
///     index: 0,
///     keyword: rstest_bdd::StepKeyword::Given,
///     text: "missing".into(),
///     feature_path: "notes/a.md".into(),
///     scenario_name: "demo".into(),
/// });
/// assert_eq!(kind, FailureKind::Undefined);
/// ```
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
#[non_exhaustive]
pub enum FailureKind {
    /// No registered step matched the invocation.
    Undefined,
    /// A required fixture was absent from the context.
    MissingFixture,
    /// The step body returned an error, typically an assertion.
    Assertion,
    /// The step body panicked.
    Panic,
    /// A failure this version does not classify more precisely.
    Other,
}

impl FailureKind {
    /// Project an [`ExecutionError`] onto its classification.
    ///
    /// Total because of the trailing wildcard arm below, not because of
    /// `#[non_exhaustive]`: that attribute is declared on `ExecutionError` in
    /// this same crate, where it does not restrict matching, and the compiler
    /// would otherwise require every variant to be named here. The wildcard is
    /// what makes an unrecognized — or newly added — variant classify silently
    /// as [`Other`](Self::Other). A variant that deserves its own
    /// classification therefore has to be added to this match deliberately;
    /// nothing will fail to compile if it is not.
    ///
    /// # Examples
    ///
    /// ```
    /// use rstest_bdd::runner::FailureKind;
    ///
    /// let kind = FailureKind::of(&rstest_bdd::execution::ExecutionError::Skip { message: None });
    /// assert_eq!(kind, FailureKind::Other);
    /// ```
    #[must_use]
    pub fn of(error: &ExecutionError) -> Self {
        match error {
            ExecutionError::StepNotFound { .. } => Self::Undefined,
            ExecutionError::MissingFixtures(_) => Self::MissingFixture,
            ExecutionError::HandlerFailed { error, .. } => match error.as_ref() {
                StepError::MissingFixture { .. } => Self::MissingFixture,
                StepError::ExecutionError { .. } => Self::Assertion,
                StepError::PanicError { .. } => Self::Panic,
            },
            _ => Self::Other,
        }
    }
}

/// Where a terminal failure occurred.
///
/// A projection of [`ScenarioFailure`] for callers that want the site alone.
///
/// # Examples
///
/// ```
/// use rstest_bdd::runner::FailureSite;
///
/// assert_eq!(FailureSite::Step(2), FailureSite::Step(2));
/// ```
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
#[non_exhaustive]
pub enum FailureSite {
    /// The invocation at this zero-based index ended the run.
    Step(usize),
    /// The run skipped, and skip policy converts that skip into a failure.
    ForcedSkip(usize),
    /// The plan contained no invocations, so there was nothing to run.
    ///
    /// A dynamic frontend's parser can emit an empty plan from a malformed
    /// document. Treating that as a pass would be the canonical false green,
    /// so the canonical fold rejects it.
    EmptyPlan,
}

/// The failure that ended a scenario run, with its site and payload together.
///
/// Produced by
/// [`into_harness_result`](crate::runner::ScenarioOutcome::into_harness_result),
/// whose error type this is. A runner run itself reports
/// [`Skipped`](crate::runner::ScenarioStatus::Skipped) rather than failing:
/// whether a skip should fail a suite is a caller's policy decision, and the
/// fold is where that decision is made once.
///
/// # Examples
///
/// The payload is what distinguishes this from the [`FailureSite`] it projects
/// to: the site says *where* a run ended, and the variant says what ended it.
///
/// ```
/// use rstest_bdd::runner::ScenarioFailure;
///
/// // A plan with nothing to run carries no error, only the reason.
/// let failure = ScenarioFailure::EmptyPlan;
/// assert!(failure.error().is_none());
/// assert_eq!(format!("{:?}", failure.site()), "EmptyPlan");
/// ```
#[derive(Debug, Clone, PartialEq, Eq)]
#[non_exhaustive]
pub enum ScenarioFailure {
    /// The invocation at `index` ended the run.
    Step {
        /// Zero-based index of the failing invocation.
        index: usize,
        /// The error that ended the run.
        error: ExecutionError,
    },
    /// The run skipped, and `!allow_skipped && fail_on_skipped` converts that
    /// skip into a failure.
    ///
    /// The whole [`ScenarioSkip`] is carried, so a caller that treats the skip
    /// as a failure does not have to rescan `steps` and recompute policy.
    ForcedSkip(ScenarioSkip),
    /// The plan contained no invocations.
    ///
    /// A runner still reports
    /// [`Passed`](crate::runner::ScenarioStatus::Passed) for an empty plan,
    /// because running nothing is not itself an error; the fold is stricter
    /// than the status, deliberately.
    EmptyPlan,
}

impl ScenarioFailure {
    /// Render this failure, substituting `path` for the error's own copy.
    ///
    /// The single place this type's failure wording lives. [`Display`] passes
    /// `None` because a `ScenarioFailure` alone carries no plan-side location —
    /// that is [`ScenarioOutcome::terminal_source`]'s to supply — while
    /// `ScenarioOutcome`'s own `Display` passes the plan's path so the rendered
    /// string agrees with the accessors, which is what INV-7 requires of it.
    ///
    /// The `match` is exhaustive rather than carrying a wildcard arm. Adding a
    /// variant is meant to stop the build here, so that the new variant gets a
    /// rendering worded for it rather than a generic line nobody chose: the
    /// deferred `Before`/`After` hook variants (D2 option (i), EP-M4) are the
    /// known arrivals, and a wildcard would let them ship as
    /// "scenario failed", which is the message a reporter prints when it has
    /// nothing to say.
    ///
    /// [`Display`]: std::fmt::Display
    /// [`ScenarioOutcome::terminal_source`]:
    ///     crate::runner::ScenarioOutcome::terminal_source
    pub(super) fn render_with(&self, path: Option<&str>) -> String {
        match self {
            Self::Step { index, error } => {
                let rendered = crate::localization::with_loader(|loader| {
                    error.format_with_loader_at(loader, path)
                });
                format!("scenario failed at step {index}: {rendered}")
            }
            Self::ForcedSkip(skip) => format!("scenario failed: skipped at step {}", skip.at()),
            Self::EmptyPlan => "scenario failed: the plan contained no steps".to_owned(),
        }
    }

    /// Project this failure onto its site.
    ///
    /// # Examples
    ///
    /// ```
    /// use rstest_bdd::runner::{FailureSite, ScenarioFailure};
    ///
    /// assert_eq!(ScenarioFailure::EmptyPlan.site(), FailureSite::EmptyPlan);
    /// ```
    #[must_use]
    pub fn site(&self) -> FailureSite {
        match self {
            Self::Step { index, .. } => FailureSite::Step(*index),
            Self::ForcedSkip(skip) => FailureSite::ForcedSkip(skip.at()),
            Self::EmptyPlan => FailureSite::EmptyPlan,
        }
    }

    /// Borrow the step error, when this is a step failure.
    ///
    /// # Examples
    ///
    /// ```
    /// use rstest_bdd::runner::ScenarioFailure;
    ///
    /// assert!(ScenarioFailure::EmptyPlan.error().is_none());
    /// ```
    #[must_use]
    pub const fn error(&self) -> Option<&ExecutionError> {
        match self {
            Self::Step { error, .. } => Some(error),
            _ => None,
        }
    }
}

/// Render the failure.
///
/// This is the error type of the canonical
/// [`into_harness_result`](crate::runner::ScenarioOutcome::into_harness_result),
/// so it is what a caller's own failure message will most often format, and a
/// `Display` is what makes that possible in a `panic!("{failure}")` or a
/// `Result` with `?`.
///
/// The rendering falls back to the error's own recorded path, because a
/// [`ScenarioFailure`] carries no plan-side location to prefer. A caller that
/// holds the outcome should render *that* — `ScenarioOutcome`'s `Display`
/// substitutes the plan's path, which is the rendering INV-7 governs.
impl std::fmt::Display for ScenarioFailure {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(&self.render_with(None))
    }
}

/// The failure as an error source.
///
/// A step failure's cause is the [`ExecutionError`] that ended the run, so it
/// is exposed as the source; the other two variants have no cause to name.
/// [`EmptyPlan`](ScenarioFailure::EmptyPlan) is a fact about the plan rather
/// than a defect to unwrap, and a forced skip's cause is the skip itself, which
/// is not an `Error`.
impl std::error::Error for ScenarioFailure {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        match self {
            Self::Step { error, .. } => Some(error),
            _ => None,
        }
    }
}
