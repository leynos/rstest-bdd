//! The async driver's attribution, and the one decision it alone can falsify.
//!
//! `run_scenario_async` attaches the scenario span with `Instrument` rather than
//! entering it with a guard, because a guard held across `.await` is
//! thread-local and would report this scenario as current on a thread that had
//! moved on (AGENTS.md forbids the form outright). Every other D14 test drives
//! the *synchronous* runner, so none of them would notice the span being
//! dropped from this path.
//!
//! "Attributes" is meant literally, and that is why this test reads
//! [`CURRENT_SPAN_FIELD`] rather than merely checking that both events exist.
//! Presence alone does not distinguish an event emitted *inside* the
//! instrumented future from one emitted outside it: a driver that dropped the
//! span would still emit both, and a test asserting only that they arrived
//! would pass against it. The attribution is the entire claim, so the
//! attribution is what is asserted.
//!
//! Split from the parent file to stay inside the repository's 400-line cap,
//! along the seam the parent's own module note already draws: the parent holds
//! the tests that drive the synchronous runner, and this file holds the one
//! that drives the asynchronous one.

use rstest_bdd::{
    StepContext,
    StepKeyword,
    runner::{ScenarioScope, ScenarioStatus, run_scenario_async},
};
use tracing::Level;

use super::{
    capture::{
        CURRENT_SPAN_FIELD,
        capture,
        carrying,
        per_step_events,
        scenario_span_values,
        seen,
        value,
    },
    plan,
};

/// The asynchronous driver attributes its events to the scenario span.
#[test]
fn the_async_driver_attributes_its_events_to_the_scenario_span() {
    let (captured, _guard) = capture(Level::TRACE);
    let mut ctx = StepContext::default();
    let plan = plan(&[(StepKeyword::Given, "an instrumented step passes", 43)]);

    let runtime = tokio::runtime::Builder::new_current_thread()
        .build()
        .expect("a current-thread runtime builds");
    let outcome = runtime.block_on(run_scenario_async(&plan, ScenarioScope::new(&mut ctx)));
    assert_eq!(
        outcome.status(),
        ScenarioStatus::Passed,
        "the step must resolve under the async driver too, or this observes the wrong run",
    );

    let values = scenario_span_values(&captured);
    assert_eq!(value(&values, "name"), "Instrumented");

    let all = seen(&captured);

    // The span itself is opened outside any other span, so it is attributed to
    // nothing. Asserting this pins the synthetic field's meaning: it reports the
    // *enclosing* span, and a subscriber that attributed every capture to the
    // nearest span by name would report `scenario` here too and make the two
    // assertions below unfalsifiable.
    let scenario = carrying(&all, "name");
    assert_eq!(scenario.name, "scenario");
    assert!(
        !scenario.carries(CURRENT_SPAN_FIELD),
        "the `scenario` span is the outermost span the run opens, so nothing encloses it; \
         captured {scenario:?}",
    );

    // The assertion that carries the `Instrument` decision. The policy event is
    // emitted inside the instrumented future, so if the driver opened the span
    // without keeping it current across the poll this would be missing — and it
    // is the attribution, not the event's presence, that goes missing.
    let resolved = carrying(&all, "forced_failure");
    assert_eq!(value(&resolved.values, "plan_allows_skipping"), "false");
    assert_eq!(
        value(&resolved.values, CURRENT_SPAN_FIELD),
        "scenario",
        "the policy event must be attributed to the scenario span, not merely emitted",
    );

    // Every per-step event, not just the first: a driver that instrumented only
    // the first poll would leave the later events unattributed while still
    // emitting them, which a single check could not see.
    let steps = per_step_events(&all);
    assert!(
        !steps.is_empty(),
        "the plan records an invocation, so a per-step event must exist; captured {all:?}",
    );
    for step in &steps {
        assert_eq!(
            value(&step.values, CURRENT_SPAN_FIELD),
            "scenario",
            "every per-step event belongs to the scenario span; the unattributed one was {step:?}",
        );
    }
}
