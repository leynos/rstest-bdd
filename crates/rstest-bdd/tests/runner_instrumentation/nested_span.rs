//! The nesting branch of the capture subscriber, and the witness that reaches it.
//!
//! [`CapturingSubscriber::new_span`](super::capture) records the enclosing span
//! in [`CURRENT_SPAN_FIELD`] when there is one. The runner never opens a span
//! inside another — `drive_async`'s `scenario` span is the outermost thing it
//! creates, and the synchronous sibling opens none above it — so on every other
//! test in this suite that branch goes untaken. An untaken branch cannot be told
//! apart from one that was never written: the parent's assertion that the
//! `scenario` span carries no enclosing span passes just as well when the field
//! is never populated for *any* span.
//!
//! This file supplies the missing witness. It opens a span of its own around the
//! run, so `scenario` does have an enclosing span and the field must be filled
//! from it. The two tests are then a discriminating pair — the witness proves
//! the field is written when a parent exists, and the parent's assertion proves
//! it is left empty when none does — and neither alone would pin the meaning.
//!
//! Its own file rather than a second test in `async_attribution.rs` because the
//! subject is the *capture* and not the driver: nothing here would change if the
//! async driver's `Instrument` decision were reversed.

use rstest_bdd::{
    StepContext,
    StepKeyword,
    runner::{ScenarioScope, run_scenario_async},
};
use tracing::Level;

use super::{
    capture::{CURRENT_SPAN_FIELD, capture, carrying, seen, value},
    plan,
};

/// A scenario run inside another span is attributed to that span.
#[test]
fn a_nested_scenario_span_records_its_enclosing_span() {
    let (captured, _guard) = capture(Level::TRACE);
    let mut ctx = StepContext::default();
    let plan = plan(&[(StepKeyword::Given, "an instrumented step passes", 43)]);

    let runtime = tokio::runtime::Builder::new_current_thread()
        .build()
        .expect("a current-thread runtime builds");

    // `in_scope` enters the span for the closure and leaves it again, so the
    // driver opens `scenario` while this span is current — which is the
    // condition `new_span` reads. An `Entered` guard would work here too, since
    // nothing suspends across a thread boundary, but the guard form is what
    // AGENTS.md forbids and the closure form makes the boundary explicit. The
    // future is driven inside the scope, not merely built there.
    let outer = tracing::info_span!("outer");
    let outcome = outer
        .in_scope(|| runtime.block_on(run_scenario_async(&plan, ScenarioScope::new(&mut ctx))));
    assert_eq!(
        format!("{:?}", outcome.status()),
        "Passed",
        "the step must resolve, or this observes the wrong run",
    );

    // Looked up by its `name` field, so `outer` — which carries no `name` — is
    // skipped and the capture found is the `scenario` span rather than the span
    // this test opened.
    let all = seen(&captured);
    let scenario = carrying(&all, "name");
    assert!(
        scenario.carries(CURRENT_SPAN_FIELD),
        "the `scenario` span was opened inside `outer`, so the enclosing span must have been \
         recorded; captured {scenario:?}",
    );
    assert_eq!(
        value(&scenario.values, CURRENT_SPAN_FIELD),
        "outer",
        "the recorded enclosing span must be the one that was current, and the parent test's \
         empty-field assertion is only meaningful once this branch is known to be live",
    );
}
