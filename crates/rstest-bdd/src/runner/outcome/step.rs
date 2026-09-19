//! Per-step outcome types.
//!
//! The recorded result of one invocation. Status and payload are stored as one
//! private sum, so a `Passed` outcome carrying an error is unrepresentable
//! rather than merely untested.

use crate::{ExecutionError, StepKeyword, runner::source::SourceLocation};

/// The status of one step invocation.
///
/// Fieldless and `#[non_exhaustive]`, so the representation behind
/// [`StepOutcome`] can move freely.
///
/// # Examples
///
/// ```
/// use rstest_bdd::runner::StepStatus;
///
/// assert_ne!(StepStatus::Passed, StepStatus::Bypassed);
/// ```
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
#[non_exhaustive]
pub enum StepStatus {
    /// The step ran and succeeded.
    Passed,
    /// The step requested that the scenario be skipped.
    Skipped,
    /// The step ran and failed.
    Failed,
    /// The step never ran, because an earlier step was terminal.
    Bypassed,
}

/// What became of a step's returned value.
///
/// A projection of [`InsertOutcome`](crate::InsertOutcome) that drops the
/// displaced previous override, which cannot be compared and which the outcome
/// has no use for. This is the only signal that a returned value reached no
/// later step: the runtime warns for
/// [`AmbiguousIgnored`](Self::AmbiguousIgnored) but is silent for
/// [`NoMatch`](Self::NoMatch).
///
/// # Examples
///
/// ```
/// use rstest_bdd::runner::ValueFate;
///
/// assert_ne!(ValueFate::Inserted, ValueFate::NoMatch);
/// ```
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
#[non_exhaustive]
pub enum ValueFate {
    /// The value was recorded as an override for the uniquely matching fixture.
    Inserted,
    /// No fixture matched the value's type; the value was dropped.
    NoMatch,
    /// More than one fixture matched; the value was dropped as ambiguous.
    AmbiguousIgnored,
}

impl From<crate::InsertOutcome> for ValueFate {
    fn from(outcome: crate::InsertOutcome) -> Self {
        match outcome {
            crate::InsertOutcome::Inserted(_) => Self::Inserted,
            crate::InsertOutcome::NoMatch => Self::NoMatch,
            crate::InsertOutcome::AmbiguousIgnored => Self::AmbiguousIgnored,
        }
    }
}

/// The status and payload of one recorded invocation.
///
/// One private sum rather than public fields, so the payload can never
/// contradict the status.
///
/// Built only by the engine, which lands in EP-M2; until then the unit tests
/// construct records through `StepOutcome`'s constructors. See
/// [`ScenarioSkip::new`](crate::runner::ScenarioSkip::new) for why the
/// expectation is `not(test)`-scoped.
#[derive(Debug, Clone, PartialEq, Eq)]
enum StepRecord {
    /// The step ran and succeeded, optionally having returned a value.
    Passed {
        /// What became of the returned value, when the step returned one.
        value: Option<ValueFate>,
    },
    /// The step requested a skip, carrying its optional reason.
    Skipped {
        /// The reason the step supplied, when it supplied one.
        message: Option<String>,
    },
    /// The step ran and failed.
    Failed {
        /// The error that ended the step.
        error: Box<ExecutionError>,
    },
    /// The step never ran.
    Bypassed,
}

/// The recorded result of one invocation, in plan order.
///
/// Every terminal [`ScenarioOutcome`](crate::runner::ScenarioOutcome) contains
/// exactly one `StepOutcome` per planned invocation, in plan order, with every
/// entry after a terminal event [`Bypassed`](StepStatus::Bypassed).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct StepOutcome {
    /// Zero-based position of this invocation in the plan.
    index: usize,
    /// The keyword the plan recorded.
    keyword: StepKeyword,
    /// The step text the plan recorded.
    text: String,
    /// Where the invocation was written, when the plan recorded it.
    source: Option<SourceLocation>,
    /// What actually happened.
    record: StepRecord,
}

impl StepOutcome {
    /// Record a successful invocation.
    ///
    /// The production caller is the runner's engine, which lands in EP-M2; the
    /// unit tests build outcomes through these constructors so the fold can be
    /// tested without a registry. See
    /// [`ScenarioSkip::new`](crate::runner::ScenarioSkip::new) for why the
    /// expectation is `not(test)`-scoped.
    pub(crate) fn passed(
        index: usize,
        keyword: StepKeyword,
        text: &str,
        source: Option<&SourceLocation>,
        value: Option<ValueFate>,
    ) -> Self {
        Self {
            index,
            keyword,
            text: text.to_owned(),
            source: source.cloned(),
            record: StepRecord::Passed { value },
        }
    }

    /// Record an invocation that requested a skip.
    ///
    /// See [`passed`](Self::passed) for why the expectation is `not(test)`-scoped.
    pub(crate) fn skipped(
        index: usize,
        keyword: StepKeyword,
        text: &str,
        source: Option<&SourceLocation>,
        message: Option<String>,
    ) -> Self {
        Self {
            index,
            keyword,
            text: text.to_owned(),
            source: source.cloned(),
            record: StepRecord::Skipped { message },
        }
    }

    /// Record an invocation that failed.
    ///
    /// See [`passed`](Self::passed) for why the expectation is `not(test)`-scoped.
    pub(crate) fn failed(
        index: usize,
        keyword: StepKeyword,
        text: &str,
        source: Option<&SourceLocation>,
        error: ExecutionError,
    ) -> Self {
        Self {
            index,
            keyword,
            text: text.to_owned(),
            source: source.cloned(),
            record: StepRecord::Failed {
                error: Box::new(error),
            },
        }
    }

    /// Record an invocation that never ran.
    ///
    /// See [`passed`](Self::passed) for why the expectation is `not(test)`-scoped.
    pub(crate) fn bypassed(
        index: usize,
        keyword: StepKeyword,
        text: &str,
        source: Option<&SourceLocation>,
    ) -> Self {
        Self {
            index,
            keyword,
            text: text.to_owned(),
            source: source.cloned(),
            record: StepRecord::Bypassed,
        }
    }

    /// Return the zero-based position of this invocation in the plan.
    #[must_use]
    pub const fn index(&self) -> usize { self.index }

    /// Return the keyword the plan recorded for this invocation.
    #[must_use]
    pub const fn keyword(&self) -> StepKeyword { self.keyword }

    /// Borrow the step text the plan recorded, without its keyword.
    #[must_use]
    pub fn text(&self) -> &str { &self.text }

    /// Borrow where the invocation was written, when the plan recorded it.
    ///
    /// Available for every status, including
    /// [`Bypassed`](StepStatus::Bypassed).
    #[must_use]
    pub const fn source(&self) -> Option<&SourceLocation> { self.source.as_ref() }

    /// Return what happened to this invocation.
    #[must_use]
    pub const fn status(&self) -> StepStatus {
        match self.record {
            StepRecord::Passed { .. } => StepStatus::Passed,
            StepRecord::Skipped { .. } => StepStatus::Skipped,
            StepRecord::Failed { .. } => StepStatus::Failed,
            StepRecord::Bypassed => StepStatus::Bypassed,
        }
    }

    /// Borrow the reason the step supplied, when it requested a skip with one.
    #[must_use]
    pub fn skip_message(&self) -> Option<&str> {
        match &self.record {
            StepRecord::Skipped { message } => message.as_deref(),
            _ => None,
        }
    }

    /// Borrow the error that ended the step, when it failed.
    #[must_use]
    pub fn error(&self) -> Option<&ExecutionError> {
        match &self.record {
            StepRecord::Failed { error } => Some(error),
            _ => None,
        }
    }

    /// Project a failure onto a small stable classification.
    ///
    /// A reporter can branch on this instead of matching
    /// [`ExecutionError`]'s variants directly, which keeps extension of the
    /// error representation from freezing reporter code.
    #[must_use]
    pub fn failure_kind(&self) -> Option<crate::runner::FailureKind> {
        self.error().map(crate::runner::FailureKind::of)
    }

    /// Return what became of this step's returned value, when it returned one.
    ///
    /// [`ValueFate::NoMatch`] means the value reached no later step. The
    /// runtime emits no warning for it, so this is the only signal.
    #[must_use]
    pub const fn value_insertion(&self) -> Option<ValueFate> {
        match self.record {
            StepRecord::Passed { value } => value,
            _ => None,
        }
    }
}
