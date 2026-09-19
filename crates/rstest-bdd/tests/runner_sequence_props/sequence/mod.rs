//! The vocabulary the sequence suite speaks, and the modules that use it.
//!
//! Everything here is plain data: the [`Kind`]s a generated invocation may
//! name, the [`Arrangement`]s the context may be built with, and the
//! [`Reading`] one observer's look at the probe produces. The run harness and
//! its predicates live in `sequence/run.rs`, the non-vacuity accumulator in
//! `sequence/witnesses.rs`, the strategy in `sequence/generator.rs`, and the
//! registered steps in `sequence/steps/`. The properties themselves are in
//! `runner_sequence_props.rs`; the split keeps every file under the
//! repository's 400-line cap, which `scripts/check_rs_file_lengths.py` applies
//! to integration tests as well as to library code.
//!
//! The vocabulary is what stays here rather than travelling with any one
//! consumer, because the generator, the harness, the accumulator, and the
//! failure messages all speak it: a kind's *text* is decided here and rendered
//! in a `describe` there, and putting either half elsewhere would make the
//! other reach back across a module boundary for it.
//!
//! # The shape of a case
//!
//! A generated case is a plan's invocations — each tagged with the registered
//! step that answers it — paired with an [`Arrangement`] describing the probe
//! fixture the context is built with. The two are drawn independently, which is
//! what makes all three [`ValueFate`]s reachable without a strategy per fate: a
//! value-returning step under one fixture is `Inserted`, under none is
//! `NoMatch`, and under two is `AmbiguousIgnored`.
//!
//! # Why the steps are data rather than closures
//!
//! A step's behaviour is a registered function in a process-global registry
//! the generator cannot reach and should not try to. [`Kind`] only describes
//! *which* registered function a generated plan names. Keeping it a plain enum
//! means the generator, the predicates, and the failure messages speak one
//! vocabulary, and adding a kind is a compile error at every `match` rather
//! than a silently unreachable case.

use rstest_bdd::{
    StepKeyword,
    runner::{FailureKind, ScenarioStatus},
};

mod generator;
mod run;
pub(crate) mod steps;
mod witnesses;

pub(crate) use generator::case;
pub(crate) use run::{Run, context_for, run_case, run_case_async};
/// The pattern text the generator names, and the placeholder helper.
///
/// Re-exported at this level so the properties, the generator, and the
/// context builder all reach one set of strings by one path. The
/// registrations themselves have to spell their own literals — a step
/// attribute accepts no path — and the agreement between the two is checked
/// by the runtime; see `steps/names.rs`.
///
/// This is a plain re-export rather than a `pub(crate) mod names { .. }` shim:
/// Whitaker's `module_must_have_inner_docs` requires an inner doc comment on
/// every module, and a shim module holding a single re-export would need a
/// paragraph of prose that said nothing the re-export itself does not.
/// `steps::names` keeps its own documentation, and this re-export carries a
/// doc comment because a `pub(crate) use` with none is itself a lint.
pub(crate) use steps::names;
pub(crate) use steps::{executed, observed, reset_logs};
pub(crate) use witnesses::Witnesses;

/// The value a probe fixture starts at, before any step inserts over it.
///
/// `usize::MAX` rather than `0`, because `0` is a legal producer index: an
/// observer that ran *before* producer `0` and one that ran *after* it would
/// otherwise read the same number, and INV-3's "and to no invocation `j <= i`"
/// clause would be unfalsifiable.
pub(crate) const SENTINEL: usize = usize::MAX;

/// The most invocations a generated plan may contain.
///
/// INV-1's domain says "length 0 to 8". Zero is included so the empty plan —
/// INV-13's shape — is generated rather than sampled only when the draw happens
/// to be empty.
pub(crate) const MAX_STEPS: usize = 8;

/// The number of cases each property runs.
///
/// Pinned in-file rather than left to `Config::default()`: nextest runs with
/// `slow-timeout = { period = "60s", terminate-after = 1 }`, and one case is a
/// real scenario run through the registry. At this budget the binary stays far
/// inside that period even when every case generates [`MAX_STEPS`] invocations
/// and every invocation panics.
pub(crate) const CASES: u32 = 256;

/// Which registered step a generated invocation names.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum Kind {
    /// Resolves and does nothing.
    Pass,
    /// Resolves and returns a probe carrying its producing index.
    ReturnValue,
    /// Resolves and returns a value no fixture can match by type.
    ReturnUnmatchedValue,
    /// Resolves and records what the probe name resolves to.
    Observe,
    /// Resolves and asks to be skipped.
    Skip,
    /// Resolves and returns an error.
    HandlerError,
    /// Resolves to nothing, because no step is registered for its text.
    UnregisteredStep,
    /// Resolves to a step that declares a fixture the context does not hold.
    MissingFixture,
    /// Resolves and panics.
    Panic,
}

impl Kind {
    /// Every kind, for the generator and the completeness assertions.
    pub(crate) const ALL: [Self; 9] = [
        Self::Pass,
        Self::ReturnValue,
        Self::ReturnUnmatchedValue,
        Self::Observe,
        Self::Skip,
        Self::HandlerError,
        Self::UnregisteredStep,
        Self::MissingFixture,
        Self::Panic,
    ];

    /// Whether an invocation of this kind hands a value back to the driver.
    pub(crate) const fn returns_a_value(self) -> bool {
        matches!(self, Self::ReturnValue | Self::ReturnUnmatchedValue)
    }

    /// Whether this kind's handler runs at all, and so logs its position.
    ///
    /// [`Self::MissingFixture`] fails fixture validation before its handler is
    /// called and [`Self::UnregisteredStep`] never resolves, so neither can
    /// contribute to the execution log INV-1 is checked against. Every other
    /// kind reaches a handler and records.
    pub(crate) const fn logs_its_position(self) -> bool {
        !matches!(self, Self::MissingFixture | Self::UnregisteredStep)
    }

    /// The keyword this kind's step is registered under.
    ///
    /// Part of the kind rather than of the invocation's position, because
    /// `resolve_step` filters on keyword equality: a step registered under
    /// `Then` and invoked under `When` resolves to nothing just as surely as
    /// one that was never registered. Assigning keywords by position instead
    /// would make most generated invocations unresolvable and collapse every
    /// classification into `Undefined`.
    pub(crate) const fn keyword(self) -> StepKeyword {
        match self {
            Self::Pass | Self::Skip | Self::MissingFixture => StepKeyword::Given,
            Self::ReturnValue | Self::ReturnUnmatchedValue | Self::Observe => StepKeyword::When,
            Self::HandlerError | Self::UnregisteredStep | Self::Panic => StepKeyword::Then,
        }
    }

    /// The pattern text for this kind, given the invocation's index.
    ///
    /// Every kind registered with a `{index:usize}` placeholder has it filled
    /// with the invocation's own index, because each of those handlers logs the
    /// position it ran at. That is what makes INV-1's execution log a *complete*
    /// record: an earlier revision logged only from the value-returning and
    /// observing kinds, leaving a trailing ordinary invocation with no trace at
    /// all, so a driver that ran one past its terminal would go unnoticed.
    ///
    /// The two kinds with no placeholder are the two that cannot run: nothing
    /// is registered for [`Kind::UnregisteredStep`], and
    /// [`Kind::MissingFixture`] fails validation before its handler is reached.
    /// Neither ever reaches a handler, so neither has a position to log.
    ///
    /// For [`Kind::ReturnValue`] the index is also what the returned probe
    /// carries, so an observer's reading names the producer it saw. For
    /// [`Kind::Observe`] it is what the observer records beside its reading, so
    /// a predicate never has to infer an observer's position from the order the
    /// observers happened to run in.
    pub(crate) fn text(self, index: usize) -> String {
        match self {
            Self::Pass => names::substitute(names::PASSES, index),
            Self::ReturnValue => names::substitute(names::RETURN, index),
            Self::ReturnUnmatchedValue => names::substitute(names::RETURN_UNMATCHED, index),
            Self::Observe => names::substitute(names::OBSERVE, index),
            Self::Skip => names::substitute(names::SKIPS, index),
            Self::HandlerError => names::substitute(names::FAILS, index),
            Self::UnregisteredStep => names::UNREGISTERED.to_owned(),
            Self::MissingFixture => names::MISSING_FIXTURE.to_owned(),
            Self::Panic => names::substitute(names::PANICS, index),
        }
    }

    /// The status a run ends with when this kind is its terminal invocation.
    ///
    /// Declared beside the kinds themselves so a new kind cannot be added
    /// without answering the question, and asserted by the per-kind rows in
    /// `runner_sequence_props.rs` so the answer is checked rather than trusted.
    pub(crate) const fn terminal_status(self) -> Option<ScenarioStatus> {
        match self {
            Self::Pass | Self::ReturnValue | Self::ReturnUnmatchedValue | Self::Observe => None,
            Self::Skip => Some(ScenarioStatus::Skipped),
            Self::HandlerError | Self::UnregisteredStep | Self::MissingFixture | Self::Panic => {
                Some(ScenarioStatus::Failed)
            }
        }
    }

    /// The classification a run's terminal failure must carry, if it fails.
    pub(crate) const fn failure_kind(self) -> Option<FailureKind> {
        match self {
            Self::Pass
            | Self::ReturnValue
            | Self::ReturnUnmatchedValue
            | Self::Observe
            | Self::Skip => None,
            Self::HandlerError => Some(FailureKind::Assertion),
            Self::UnregisteredStep => Some(FailureKind::Undefined),
            Self::MissingFixture => Some(FailureKind::MissingFixture),
            Self::Panic => Some(FailureKind::Panic),
        }
    }
}

/// One invocation in a generated plan.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct Step {
    /// Which registered step answers it.
    pub(crate) kind: Kind,
    /// The keyword it is invoked under.
    pub(crate) keyword: StepKeyword,
    /// The source line recorded for it, one-based.
    pub(crate) line: u32,
}

/// What the context is built with, drawn independently of the plan.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum Arrangement {
    /// No fixture of the probe's type: an insert has nothing to match.
    NoProbe,
    /// Exactly one: an insert has a unique match.
    OneProbe,
    /// Two: an insert is ambiguous and is dropped.
    TwoProbes,
}

impl Arrangement {
    /// Every arrangement, for the generator.
    pub(crate) const ALL: [Self; 3] = [Self::NoProbe, Self::OneProbe, Self::TwoProbes];

    /// The names this arrangement registers its probes under, in order.
    ///
    /// The names live here rather than only at the `insert_owned` call so that
    /// a test can state "this arrangement registers exactly these fixtures"
    /// without reaching into the builder. Two probes under two names is what
    /// INV-12's ambiguity clause needs; two under one name would overwrite and
    /// never be ambiguous at all.
    pub(crate) const fn probe_names(self) -> &'static [&'static str] {
        match self {
            Self::NoProbe => &[],
            Self::OneProbe => &[names::PROBE],
            Self::TwoProbes => &[names::PROBE, names::OTHER_PROBE],
        }
    }

    /// How many probe fixtures this arrangement registers.
    pub(crate) const fn probes(self) -> usize {
        match self {
            Self::NoProbe => 0,
            Self::OneProbe => 1,
            Self::TwoProbes => 2,
        }
    }
}

/// What one observer read, and which invocation it was.
///
/// The `PartialEq` here is load-bearing for INV-5 rather than incidental:
/// [`Run`] is compared as a whole, and a `Run` holds a `Vec<Reading>`. See
/// `sequence/run.rs`.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) struct Reading {
    /// The observer invocation's index in the plan it belongs to.
    pub(crate) observer: usize,
    /// What it read, or `None` when the name did not resolve at all.
    pub(crate) value: Option<usize>,
}
