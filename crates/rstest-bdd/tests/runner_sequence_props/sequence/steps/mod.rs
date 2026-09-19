//! The registered steps every generated plan invokes.
//!
//! The pattern text here is written as a literal, because that is the only
//! form the step attributes accept; [`names`] holds the same text for the
//! generator and the predicates to name, and the agreement between the two is
//! checked by the runtime rather than assumed. See `names.rs`.
//!
//! Each step's only side effect outside the driver is to append its own
//! invocation index to a thread-local log. That log — not `outcome.steps()` —
//! is the evidence INV-1 is checked against: `steps()` records what the driver
//! *says* it did, and a driver that executed a skipped-over step and then
//! recorded it as `Bypassed` would satisfy every assertion made from `steps()`
//! alone.
//!
//! # A step's keyword is a property of the step, not of its position
//!
//! `resolve_step` filters on keyword equality, so an invocation only resolves
//! when its keyword is the one its step was registered under. A step answered
//! by [`names::PASSES`] therefore resolves under `Given` and under nothing
//! else: invoking it under `When` finds no step and classifies as `Undefined`.
//! That is why the generator takes each invocation's keyword from its
//! [`Kind`](super::Kind) rather than from its position in the plan, and why
//! `runner_sequence_props::controls::a_keyword_mismatch_resolves_to_nothing` pins the
//! behaviour the earlier design would have walked into.
//!
//! # Why the index is a placeholder
//!
//! The generator produces a plan, not a program: the same handful of patterns
//! answer every case, and the ones that must know their own position carry it as
//! a `{index:usize}` placeholder that the generator fills with the invocation's
//! index. That is what lets a handler log where it ran without being told, and
//! what lets a returned probe carry the index of the invocation that produced
//! it — so an observer's reading identifies *which* producer it saw rather than
//! merely proving that something was inserted.
//!
//! # Why the observer reads through the context
//!
//! `StepContext::try_borrow` consults its override map first and its fixture
//! entries second, and `insert_value` writes into that override map under the
//! *fixture's* name. So an observer naming the probe reads a producer's value
//! when one was inserted and the fixture's own value otherwise — which is
//! exactly the visibility INV-3 is about. Borrowing the fixture's cell directly
//! would instead prove only that the fixture still exists, a fact that holds
//! under every fate and would make the property unfalsifiable.
//!
//! # Why this is an integration test
//!
//! D21: the unit-test binary cannot reach the registry, so a test that merely
//! *executes a step* must live in an integration binary. See `runner_wire.rs`.

use std::cell::RefCell;

use rstest_bdd::{
    StepContext,
    StepError,
    StepExecution,
    StepExecutionMode,
    StepFuture,
    StepKeyword,
    StepPattern,
    StepText,
    extract_placeholders,
    submit,
};
use rstest_bdd_macros::{given, then, when};

use super::Reading;

pub(crate) mod names;

/// The type a returning step hands back and a probe fixture holds.
///
/// One type for both roles, deliberately: `insert_value` matches the returned
/// value's `TypeId` against the fixture entries', so a returned value can only
/// find a fixture of its own type. A distinct observer-target type would make
/// every case `NoMatch` and hide the behaviour INV-3 is about.
#[derive(Debug, PartialEq, Eq)]
pub(crate) struct Probe(pub(crate) usize);

thread_local! {
    /// The invocation indices whose handlers ran, in execution order.
    static EXECUTED: RefCell<Vec<usize>> = const { RefCell::new(Vec::new()) };

    /// What each observer read, in the order the observers ran.
    static OBSERVED: RefCell<Vec<Reading>> = const { RefCell::new(Vec::new()) };
}

/// Append an invocation index to the execution log.
fn record(index: usize) { EXECUTED.with(|log| log.borrow_mut().push(index)); }

/// The invocation indices whose handlers ran, in order.
pub(crate) fn executed() -> Vec<usize> { EXECUTED.with(|log| log.borrow().clone()) }

/// What each observer read, in order.
pub(crate) fn observed() -> Vec<Reading> { OBSERVED.with(|log| log.borrow().clone()) }

/// Clear both logs. Called before each run, never between runs.
pub(crate) fn reset_logs() {
    EXECUTED.with(|log| log.borrow_mut().clear());
    OBSERVED.with(|log| log.borrow_mut().clear());
}

/// A step that resolves and does nothing but log its position.
///
/// It records like every other kind. An earlier revision logged only from the
/// returning and observing steps, which left INV-1's evidence a partial record:
/// a driver that ran a trailing invocation past its terminal, where the
/// trailing invocation happened to be an ordinary passing step, would leave no
/// trace for the predicate to find.
#[given("a sequence probe step passes {index:usize}")]
fn passes(index: usize) { record(index); }

/// A step that resolves and asks to be skipped.
///
/// `skip!` unwinds with a `SkipRequest` the wrapper catches, so this invocation
/// becomes the run's terminal index.
#[given("a sequence probe step skips {index:usize}")]
fn skips(index: usize) {
    record(index);
    rstest_bdd::skip!("the sequence asked for a skip");
}

/// A step that resolves and returns an error.
///
/// A bare `Err` classifies as `FailureKind::Assertion`. It is neither a panic
/// nor a resolution failure, so it is a distinct terminal kind for INV-1's
/// classification.
#[then("a sequence probe step fails {index:usize}")]
fn fails(index: usize) -> Result<(), &'static str> {
    record(index);
    Err("deliberate sequence failure")
}

/// A step that resolves and panics.
///
/// The macro wrapper catches the unwind and returns it as
/// `HandlerFailed`/`PanicError`, which classifies as `FailureKind::Panic`. That
/// the panic never reaches the caller is `runner_panics.rs`'s subject; what
/// matters here is that a panicking step is a terminal kind the classification
/// must reach.
#[then("a sequence probe step panics {index:usize}")]
fn panics(index: usize) {
    record(index);
    panic!("deliberate sequence panic");
}

/// A step that declares a fixture the generated context never supplies.
///
/// The `#[from]` parameter is what puts the name in the registry entry's
/// fixture list. The context is built without it, so validation fails before the
/// handler is reached and the run ends with `MissingFixtures` — a
/// resolution-time failure rather than a handler failure, and therefore a
/// distinct terminal kind.
#[given("a sequence probe step needs an absent fixture")]
fn needs_absent_fixture(#[from(absent)] _absent: &Absent) {}

/// The absent fixture's type, declared and never inserted.
///
/// No fixture function accompanies it, and none is needed: the macro records a
/// fixture requirement as a *name and a type string*, and the wrapper resolves
/// it from the `StepContext` by name and type at run time. Nothing calls a
/// fixture function by path, so a step can declare a fixture that has none —
/// which is what makes this step's failure a genuine "the context was not given
/// it" rather than a compile error. `context_for` inserts nothing under the
/// name, so validation fails and the run ends `MissingFixture`.
#[derive(Debug)]
pub(crate) struct Absent;

/// A step that returns a probe carrying the invocation that ran it.
#[when("a sequence probe step returns probe {index:usize}")]
fn returns_probe(index: usize) -> Probe {
    record(index);
    Probe(index)
}

/// A step that returns a value no generated fixture can match by type.
///
/// It logs its position like every other positional kind, and for the same
/// reason: the log INV-1 is checked against has to be a complete record of what
/// ran. An entry that was not a real invocation index would be worse than
/// useless, so the index comes from the pattern's placeholder — the position
/// the generator actually placed this invocation at — and never from anything
/// the handler invents.
#[when("a sequence probe step returns an unmatched value {index:usize}")]
fn returns_unmatched(index: usize) -> Unmatched {
    record(index);
    Unmatched
}

/// A type deliberately unlike every fixture this suite registers.
#[derive(Debug)]
pub(crate) struct Unmatched;

/// A step that records what the probe name currently resolves to.
///
/// `try_borrow` consults the override map before the fixture entries, so this
/// reads a producer's inserted value when there is one and the fixture's own
/// sentinel otherwise. `None` means the name did not resolve at all — which,
/// under an arrangement with no probe fixture, is the only thing an observer
/// *can* report, and is kept distinct from the sentinel so an unresolvable name
/// is never counted as a passing observation.
///
/// # Why this one is registered raw
///
/// An attribute step cannot take the context. The argument classifier has no
/// type-based recognition of `StepContext`: a parameter is a placeholder when
/// its name matches one, an explicit `#[from]`/`#[datatable]`/`#[step_args]`
/// when it carries that attribute, a fixture otherwise. So `ctx: &mut
/// StepContext<'_>` becomes a *fixture* named `ctx` of type `StepContext<'_>`,
/// and because nothing inserts one the wrapper reports `MissingFixtures` with
/// `required: ["ctx"]` before the handler is ever called. No spelling of the
/// attribute changes that — the repo's own context-reaching steps are all raw
/// registrations for the same reason.
///
/// The raw form needs no classifier, because it never parses a signature: the
/// handler is a plain [`StepFn`](rstest_bdd::StepFn) this module writes out.
/// That also makes the placeholder explicit, since there is no generated
/// binding to parse it. So the index is recovered from the invocation text with
/// the same [`extract_placeholders`](rstest_bdd::extract_placeholders) the
/// registry itself matches with, and a text that fails to yield exactly one
/// capture is a hard error rather than a silent zero — a fabricated index would
/// put an invocation that never ran into the very log INV-1 is checked against.
fn observes_probe(
    ctx: &mut StepContext<'_>,
    text: &str,
    _docstring: Option<&str>,
    _table: Option<&[&[&str]]>,
) -> Result<StepExecution, StepError> {
    let observer = placeholder_index(text)?;
    record(observer);
    let value = read_probe(ctx);
    OBSERVED.with(|log| log.borrow_mut().push(Reading { observer, value }));
    Ok(StepExecution::from_value(None))
}

/// Recover the bounded context out of an observer invocation's text.
///
/// A capture that is absent, not a `usize`, or ambiguous is reported as
/// [`StepError::ExecutionError`] rather than folded to a default. The `Err`
/// variant is chosen over `MissingFixture` because nothing about a fixture is
/// wrong; `ExecutionError` is also what a step's own `Err` maps to, so a
/// malformed capture classifies as `FailureKind::Assertion` and fails the case
/// loudly instead of passing with an invented index.
fn placeholder_index(text: &str) -> Result<usize, StepError> {
    let failure = |detail: &str| {
        Err(StepError::ExecutionError {
            pattern: names::OBSERVE.to_owned(),
            function: "observes_probe".to_owned(),
            message: format!("the observer's index is not recoverable from its text: {detail}"),
        })
    };
    let Ok(captures) = extract_placeholders(&OBSERVER_PATTERN, StepText::from(text)) else {
        return failure(text);
    };
    let [capture] = captures.as_slice() else {
        return failure(&format!(
            "expected exactly one capture, got {}",
            captures.len()
        ));
    };
    capture
        .parse()
        .map_err(|_| format!("capture {capture:?} is not a usize"))
        .or_else(|detail| failure(&detail))
}

/// The pattern `observes_probe` is registered under.
///
/// A module-level `static` because `Step` stores a `&'static StepPattern`, and
/// `extract_placeholders` needs the same pattern object the registration uses —
/// borrowing it from here rather than compiling a second one is what keeps the
/// capture this reads and the text that resolved it in agreement.
static OBSERVER_PATTERN: StepPattern = StepPattern::new(names::OBSERVE);

const _: () = {
    /// The async arm. This observer has no async body, so it runs the sync
    /// handler; `Both` in the registration below is what tells the driver it
    /// may call it from either runtime.
    fn observer_async<'ctx>(
        ctx: &'ctx mut StepContext<'_>,
        text: &'ctx str,
        docstring: Option<&'ctx str>,
        table: Option<&'ctx [&'ctx [&'ctx str]]>,
    ) -> StepFuture<'ctx> {
        Box::pin(std::future::ready(observes_probe(
            ctx, text, docstring, table,
        )))
    }

    submit! {
        rstest_bdd::Step {
            keyword: StepKeyword::When,
            pattern: &OBSERVER_PATTERN,
            run: observes_probe,
            run_async: observer_async,
            execution_mode: StepExecutionMode::Both,
            fixtures: &[],
            file: file!(),
            line: line!(),
        }
    }
};

/// Read the probe name, mapping every borrow failure to `None`.
///
/// `try_borrow` is deliberately used rather than `borrow_ref`: it reports the
/// reason a name did not resolve, and mapping that to `None` here is what keeps
/// "the name was absent" distinguishable from "the name resolved to the
/// sentinel", which the `NoMatch` fate depends on. The two differ for an
/// `Arrangement` that registers no probe at all, where `None` is the honest
/// answer and the sentinel would be a fabrication.
fn read_probe(ctx: &StepContext<'_>) -> Option<usize> {
    match ctx.try_borrow::<Probe>(names::PROBE) {
        Ok(cell) => Some(cell.0),
        Err(_) => None,
    }
}

#[cfg(test)]
mod tests {
    //! Witnesses that a fixture of the expected type is reachable by name.
    //!
    //! The arrangement-independent parts of the observation contract. Every
    //! invariant this suite asserts is read through [`read_probe`], so a
    //! fixture that stops being registered or a name that stops resolving would
    //! turn the suite into a description of nothing. Asserting it here — where
    //! the same helper is called directly — means a fixture whose type or name
    //! drifted fails a test that names it, rather than making INV-3 and INV-12
    //! unfalsifiable without saying so.
    //!
    //! The fate-per-arrangement contract is asserted in `named_witnesses.rs`
    //! instead, because [`ValueFate`](rstest_bdd::runner::ValueFate) is read from
    //! a `Run` and would mean routing these cases through the driver for no gain.
    //!
    //! An assertion here that reads a sentinel must compare against
    //! [`SENTINEL`](crate::sequence::SENTINEL) rather than spell `usize::MAX`, or
    //! it stops noticing a change to the value the fixtures are actually built
    //! with.

    use rstest_bdd::StepContext;

    use super::{Probe, names, read_probe};
    use crate::sequence::{Arrangement, SENTINEL, context_for};

    /// A probe fixture is reachable, by name, at the type the steps use.
    ///
    /// The cells are built from the arrangement's own name list rather than
    /// written out, so a second fixture added to an arrangement is supplied
    /// here too. `context_for` asserts that it was given exactly the cells the
    /// arrangement declares, so a handwritten list would fail there for a
    /// reason that has nothing to do with what this test is about.
    #[test]
    fn a_probe_fixture_resolves_by_name_and_type() {
        for arrangement in [Arrangement::OneProbe, Arrangement::TwoProbes] {
            let cells: Vec<_> = arrangement
                .probe_names()
                .iter()
                .map(|_| StepContext::owned_cell(Probe(SENTINEL)))
                .collect();
            let ctx = context_for(arrangement, &cells);
            assert_eq!(
                read_probe(&ctx),
                Some(SENTINEL),
                "({arrangement:?}) the probe fixture must resolve under {}",
                names::PROBE,
            );
        }
    }

    /// With no probe fixture registered, the name resolves to nothing.
    ///
    /// `read_probe` maps every borrow failure to `None`, so this says the name
    /// is genuinely absent rather than that the observer reported the sentinel
    /// — the difference between "no fixture here" and "a fixture reading its
    /// default", which INV-12's `NoMatch` row depends on.
    #[test]
    fn an_unregistered_probe_name_resolves_to_nothing() {
        let ctx = context_for(Arrangement::NoProbe, &[]);
        assert_eq!(
            read_probe(&ctx),
            None,
            "no probe is registered, so {} must not resolve",
            names::PROBE,
        );
    }
}
