//! The strategy, and the tagging that turns a drawn kind into an invocation.
//!
//! # What the strategy has to reach
//!
//! INV-1 needs every terminal kind to occur, INV-3 needs an observer placed
//! before a producer *and* an observer that actually reads one, and INV-12
//! needs every returned value's fate recorded. None of those is guaranteed by a
//! uniform draw: a value-returning invocation is one kind in nine, so a plan
//! long enough to hold it *and* an observer around it can easily be missed, and
//! a suite whose non-vacuity assertions therefore fail intermittently would be
//! worse than one that never made them.
//!
//! So a case is drawn as a four-tuple: a bias flag, a crafted subset, an
//! arrangement, and a uniform backdrop. The bias flag *selects* between the two
//! kind lists rather than combining them — when it is set the plan is the
//! crafted subset alone, and when it is clear the plan is the backdrop alone.
//! Every drawn kind is a member of [`Kind::ALL`], so the crafted shapes witness
//! the domain without leaving it — and the uniform half keeps every kind
//! reachable at every length rather than only through a crafted shape.
//!
//! # Why the crafted subsets are built from [`Kind::ALL`]
//!
//! A shape names *what it witnesses* rather than which kind witnesses it. The
//! producer is found by filtering [`Kind::ALL`] for a member that returns a
//! value, and the terminal by filtering for the member whose declared
//! classification is the one the shape names — so a kind whose
//! `returns_a_value`, `terminal_status`, or `failure_kind` changed is rebuilt
//! into the shape that needs it rather than silently stop being drawn there.
//! Writing either as a literal kind would make that a silent omission instead,
//! and the omission would be *vacuous*: a crafted shape that lost its terminal
//! still satisfies every property, because a plan with nothing ending it never
//! stops early. [`Terminal::asserted`] is what turns that into a panic.
//!
//! # Why the line is increasing
//!
//! The line is one-based and strictly increasing, so a driver that renumbered
//! from zero, or copied one line onto every record, fails INV-2's identity
//! clause on every plan longer than one.

use proptest::prelude::*;
use rstest_bdd::runner::{FailureKind, ScenarioStatus};

use super::{Arrangement, Kind, MAX_STEPS, Step};

/// A strategy drawing a plan and an arrangement for it.
pub(crate) fn case() -> impl Strategy<Value = (Vec<Step>, Arrangement)> {
    (
        prop::bool::ANY,
        catalogue(),
        prop::sample::select(Arrangement::ALL.as_slice()),
        prop::collection::vec(kind(), 0..=MAX_STEPS),
    )
        .prop_map(|(biased, crafted, arrangement, backdrop)| {
            let kinds = if biased { crafted } else { backdrop };
            (with_lines(kinds), arrangement)
        })
}

/// The crafted subsets, chosen uniformly as one plan.
///
/// A plan drawn from this always carries both halves of every value-returning
/// relation: a returned value (so INV-12 has a fate to record) and an observer
/// (so INV-3 has a reading to check). Whether the observer reads the producer's
/// value or the fixture's own sentinel is left to the arrangement and to where
/// the observer falls, so all three [`ValueFate`](rstest_bdd::runner::ValueFate)s
/// stay reachable from these shapes rather than being pinned to one.
///
/// The trailing `lead` invocations are there because a producer or observer at
/// the very end of a plan is not enough on its own: INV-2's bypassed-trailing
/// clause and INV-1's "nothing runs past the terminal" clause both need an
/// invocation *after* the one that ends the run.
fn catalogue() -> impl Strategy<Value = Vec<Kind>> {
    prop::sample::select(VEC_OF_KINDS).prop_map(build)
}

/// A plan from `kinds`, each tagged with its keyword and a distinct line.
///
/// The keyword comes from the kind rather than from the invocation's position:
/// a step resolves only under the keyword it was registered with, so a keyword
/// assigned by position would make most invocations resolve to nothing and
/// collapse INV-1's classification. See
/// `runner_sequence_props::controls::a_keyword_mismatch_resolves_to_nothing`.
fn with_lines(kinds: Vec<Kind>) -> Vec<Step> {
    kinds
        .into_iter()
        .enumerate()
        .map(|(index, kind)| Step {
            kind,
            keyword: kind.keyword(),
            line: u32::try_from(index).unwrap_or(u32::MAX - 1) + 1,
        })
        .collect()
}

/// A strategy drawing every step kind uniformly.
fn kind() -> impl Strategy<Value = Kind> { prop::sample::select(Kind::ALL.as_slice()) }

/// The drawn shape of a crafted plan, as index-free parameters.
///
/// A crafted plan cannot be described by its kinds alone, because the sequence
/// that clears a crafted plan has to be one the uniform half can also draw. So
/// the draw is a whole `Vec<Kind>` out of a small checked-in catalogue and the
/// plan is *built* from its summary: how many invocations precede the terminal
/// one, how many value-returning invocations to include, whether to include an
/// observer, and how many invocations to trail. The summaries are what is
/// curated; the plans are derived, so a change to a kind's classification or
/// value-returning status cannot leave a stale literal behind.
#[derive(Debug, Clone, Copy)]
struct Shape {
    /// Invocations to emit before the first value-returning one.
    head: usize,
    /// Value-returning invocations to emit.
    producers: usize,
    /// Whether to emit an observing invocation after the producers.
    observer: bool,
    /// The classification that ends the run, at the end of the plan.
    terminal: Option<Terminal>,
    /// Non-value-returning invocations to emit first.
    lead: usize,
}

/// A terminal's classification, as the shape that names it.
///
/// A [`Kind`] would be the direct spelling and is rejected: the catalogue is
/// meant to say what each crafted plan *witnesses* — a skip, a missing fixture,
/// a panic, a handler error — and naming the classification keeps that intent
/// while [`Terminal::kind`] resolves the kind from [`Kind::ALL`]. The four
/// variants below are exactly `Skip` plus the kinds whose `failure_kind` is not
/// `None`, and [`Terminal::asserted`] refuses to build a plan whose resolved
/// kind is missing.
///
/// Spelling a `Kind` here instead would put a literal in the catalogue that no
/// filter ties to `Kind::ALL`, which is the shape of omission the module
/// documentation says the catalogue avoids.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Terminal {
    /// A step that asks to be skipped.
    Skip,
    /// A step that resolves but declares a fixture the context lacks.
    MissingFixture,
    /// A step whose handler panics.
    Panic,
    /// A step whose handler returns an error.
    HandlerError,
}

impl Terminal {
    /// The kind this classification resolves to, or `None` if none does.
    fn kind(self) -> Option<Kind> { Kind::ALL.into_iter().find(|kind| self.matches(*kind)) }

    /// Whether `kind` is the kind this classification names.
    fn matches(self, kind: Kind) -> bool {
        match self {
            Self::Skip => kind.terminal_status() == Some(ScenarioStatus::Skipped),
            Self::MissingFixture => kind.failure_kind() == Some(FailureKind::MissingFixture),
            Self::Panic => kind.failure_kind() == Some(FailureKind::Panic),
            Self::HandlerError => kind.failure_kind() == Some(FailureKind::Assertion),
        }
    }

    /// This classification resolved to a kind, or a report naming the gap.
    ///
    /// Called by `build`, because a classification whose only kind was removed
    /// from [`Kind::ALL`] would otherwise build a plan that silently lost its
    /// terminal — and a crafted shape with no terminal still passes every
    /// property, since it satisfies them trivially.
    fn asserted(self) -> Kind {
        let Some(kind) = self.kind() else {
            panic!(
                "no kind in {KIND_ALL:?} classifies as {self:?}, so the crafted shape naming it \
                 would build a plan with no terminal at all — which every property accepts \
                 vacuously",
                KIND_ALL = Kind::ALL
            );
        };
        kind
    }
}

/// The catalogue of crafted plans.
static VEC_OF_KINDS: &[Shape] = &[
    Shape {
        lead: 1,
        head: 0,
        producers: 1,
        observer: false,
        terminal: Some(Terminal::Skip),
    },
    Shape {
        lead: 2,
        head: 1,
        producers: 1,
        observer: false,
        terminal: Some(Terminal::MissingFixture),
    },
    Shape {
        lead: 1,
        head: 0,
        producers: 1,
        observer: true,
        terminal: None,
    },
    Shape {
        lead: 2,
        head: 1,
        producers: 1,
        observer: true,
        terminal: Some(Terminal::Panic),
    },
    Shape {
        lead: 1,
        head: 0,
        producers: 2,
        observer: true,
        terminal: Some(Terminal::HandlerError),
    },
];

/// Build a crafted plan from a shape.
///
/// The producer is the first member of [`Kind::ALL`] that returns a value, the
/// terminal the member its [`Terminal`] classification names, and the observer
/// [`Kind::Observe`] — which is asserted to be the only kind whose handler reads
/// the probe. So the plan survives a reordering of `ALL` and a change to which
/// kind returns a value or ends a run.
fn build(shape: Shape) -> Vec<Kind> {
    let mut kinds: Vec<Kind> = vec![Kind::Pass; shape.lead + shape.head];
    let producer = Kind::ALL
        .into_iter()
        .find(|kind| kind.returns_a_value())
        .unwrap_or(Kind::Pass);
    for _ in 0..shape.producers {
        kinds.push(producer);
    }
    if shape.observer {
        // `Kind::Observe` is named rather than derived because there is no
        // declared classification for "reads the probe" to filter on — and none
        // is needed: its handler is the only step that records a reading, so a
        // kind that stopped reading would leave INV-3's two visibility
        // witnesses unsatisfiable and fail the suite loudly rather than
        // silently.
        kinds.push(Kind::Observe);
    }
    if let Some(terminal) = shape.terminal {
        kinds.push(terminal.asserted());
    }
    kinds.truncate(MAX_STEPS);
    kinds
}
