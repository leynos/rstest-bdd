//! D14: the four `tracing` events the runner is specified to emit.
//!
//! D14 names four events and says what each must carry. Naming them is not
//! emitting them, and an event whose fields are never read is indistinguishable
//! from an event that was never written: `make test` would pass either way, and
//! so would a subscriber in production that filtered on a misspelled field
//! name. This file captures the events and asserts on their fields.
//!
//! The assertions read field *values*, not merely field names; `capture.rs`
//! says why, and what a name-only capture could not witness. The capture
//! machinery lives there too, leaving this file the plan, the registered steps,
//! and the seven assertions that are D14's obligations.
//!
//! # Why this is an integration test
//!
//! D21, as amended: any runner test that calls `run_scenario` or
//! `run_scenario_async` is an integration test. The first registry lookup
//! builds `STEP_MAP`, whose duplicate-step `assert!` fires on the pattern
//! `registry/introspection.rs` registers twice on purpose, so the unit-test
//! binary cannot reach the registry *at all* — a run there can only end as an
//! unresolvable failure. This file was first written as a unit module, on the
//! narrower reading that a non-resolving test escapes the rule; all six
//! panicked. `runner_wire.rs` drew the same conclusion for the same reason.

use rstest_bdd::{
    StepContext,
    StepKeyword,
    runner::{
        ScenarioPlan,
        ScenarioPlanBuilder,
        ScenarioScope,
        ScenarioStatus,
        run_scenario,
        run_scenario_async,
    },
};
use rstest_bdd_macros::given;
use tracing::Level;

#[path = "runner_instrumentation/capture.rs"]
mod capture;

use capture::{Captured, capture, carrying, per_step_events, scenario_span_values, seen, value};

/// A step that resolves and does nothing, so a run can reach `Passed`.
#[given("an instrumented step passes")]
fn an_instrumented_step_passes() {}

/// A step that resolves and asks to be skipped, so a run can reach `Skipped`.
///
/// The skip is raised as a panic carrying a `SkipRequest`, which the
/// macro-generated wrapper catches; `skip.rs` documents the mechanism. The
/// message is a plain literal because D14 requires the *presence* of a message
/// to be logged and never its text.
#[given("an instrumented step skips")]
fn an_instrumented_step_skips() {
    rstest_bdd::skip!("instrumentation wanted a skip");
}

/// A plan naming the steps above, with a line set on both the scenario and each
/// step.
///
/// The lines are set so the span's `line` field and the warnings' `path:line`
/// have both coordinates to render, rather than a placeholder that would make
/// the `location` assertion pass without proving anything about rendering.
fn plan(steps: &[(StepKeyword, &'static str, u32)]) -> ScenarioPlan {
    let mut builder = ScenarioPlanBuilder::new("Instrumented", "notes/instrumented.md").at_line(42);
    for (keyword, text, line) in steps {
        builder = builder.step_at(*keyword, *text, *line);
    }
    builder.build()
}

/// The span carries the plan's identity and the skip-policy inputs.
///
/// D14 names five things the span must carry: name, source path and line, step
/// count, and `allow_skipped`. All five are asserted *by value*, because a span
/// missing one of them is still opened and still named `scenario`, and a span
/// carrying the wrong value is indistinguishable from a correct one when only
/// the names are read.
#[test]
fn the_run_opens_a_span_carrying_the_plans_identity() {
    let (captured, _guard) = capture(Level::TRACE);
    let mut ctx = StepContext::default();
    let plan = plan(&[(StepKeyword::Given, "an instrumented step passes", 43)]);

    let outcome = run_scenario(&plan, ScenarioScope::new(&mut ctx));
    assert_eq!(
        outcome.status(),
        ScenarioStatus::Passed,
        "the step must resolve, or this test observes the wrong run",
    );

    let values = scenario_span_values(&captured);
    assert_eq!(value(&values, "name"), "Instrumented");
    // The span's `source` is the plan's path, not the step's — the two differ
    // here (`notes/instrumented.md`), so a span built from the step would fail.
    assert_eq!(value(&values, "source"), "notes/instrumented.md");
    // `plan.source_line()` is an `Option<u32>`, and `tracing` records an
    // `Option` through its inner type, so the form is the number alone. Reading
    // this back is what established that: it was first written `Some(42)` from
    // the field's declared type, and the capture said otherwise.
    assert_eq!(value(&values, "line"), "42");
    assert_eq!(value(&values, "steps"), "1");
    // The plan sets no `allow_skipped`, so the resolved flag is exactly
    // `!fail_on_skipped` (D10); that relation is asserted against the *policy
    // event*, which records both, not against the process-global here. What is
    // asserted here is that the span recorded a resolved boolean at all.
    let allow_skipped = value(&values, "allow_skipped");
    assert!(
        matches!(allow_skipped, "true" | "false"),
        "the span must record a resolved boolean"
    );
}

/// A resolved policy is announced once, naming both inputs and both outputs.
///
/// The inputs matter as much as the outputs. The effective flag is
/// `plan_allows_skipping || !fail_on_skipped`, so a CI-versus-local difference
/// is attributable only if the log shows *which* input differed. Recording the
/// derived flag alone answers what happened but not why, which is precisely the
/// question D14 says is otherwise unanswerable after the fact.
#[test]
fn policy_resolution_is_announced_with_its_inputs() {
    let (captured, _guard) = capture(Level::DEBUG);
    let mut ctx = StepContext::default();
    let plan = plan(&[(StepKeyword::Given, "an instrumented step passes", 43)]);

    let _outcome = run_scenario(&plan, ScenarioScope::new(&mut ctx));

    let all = seen(&captured);
    let resolved = carrying(&all, "forced_failure");
    assert_eq!(
        resolved.level,
        Level::DEBUG,
        "D14 puts policy resolution at DEBUG",
    );
    // `plan_allows_skipping` is the plan's own flag, which this test controls,
    // so it is asserted by value. The other three derive from the process-global
    // `fail_on_skipped`, which a test must not set: `config` reads it once per
    // scope and `#[serial]` gives no protection against the doctests that also
    // read it. What is asserted instead is that the three agree with each other
    // under D10's rule, `forced_failure == !allow_skipped && fail_on_skipped`,
    // which is the relation D14's event exists to make checkable after the fact.
    assert_eq!(value(&resolved.values, "plan_allows_skipping"), "false");
    let fail_on_skipped = value(&resolved.values, "fail_on_skipped");
    let allow_skipped = value(&resolved.values, "allow_skipped");
    let forced_failure = value(&resolved.values, "forced_failure");
    for (field, found) in [
        ("fail_on_skipped", fail_on_skipped),
        ("allow_skipped", allow_skipped),
        ("forced_failure", forced_failure),
    ] {
        assert!(
            found == "true" || found == "false",
            "the resolution event must record `{field}` as a resolved boolean; it recorded \
             `{found}`",
        );
    }
    // The plan is `allow_skipped(false)` by default, so the effective permission
    // is exactly `!fail_on_skipped`, and `forced_failure` is the conjunction.
    assert_eq!(
        allow_skipped,
        if fail_on_skipped == "true" {
            "false"
        } else {
            "true"
        },
        "a plan that does not allow skipping takes its permission from `fail_on_skipped` alone \
         (D10); resolved to allow_skipped={allow_skipped} with fail_on_skipped={fail_on_skipped}",
    );
    assert_eq!(
        forced_failure,
        if allow_skipped == "false" && fail_on_skipped == "true" {
            "true"
        } else {
            "false"
        },
        "`forced_failure` must be `!allow_skipped && fail_on_skipped` (D10), so it cannot be true \
         for a run that permits skipping",
    );
}

/// Every recorded invocation produces a per-step event naming its status.
///
/// Three of the four statuses are asserted together, because the interesting
/// claim is comparative: the loop emits one event per invocation whether the
/// invocation ran or not, and the status is what distinguishes them. A run
/// whose skip stops it and whose trailing step is then bypassed exercises
/// `Passed`, `Skipped`, and `Bypassed` in order, which no single-status
/// assertion can establish. The fourth, `Failed`, is covered by
/// [`the_terminal_failure_is_warned_with_its_location_and_kind`].
#[test]
fn every_recorded_invocation_emits_a_per_step_event() {
    let (captured, _guard) = capture(Level::TRACE);
    let mut ctx = StepContext::default();
    let plan = plan(&[
        (StepKeyword::Given, "an instrumented step passes", 43),
        (StepKeyword::Given, "an instrumented step skips", 44),
        (StepKeyword::Then, "an instrumented step passes", 45),
    ]);

    let outcome = run_scenario(&plan, ScenarioScope::new(&mut ctx));

    assert_eq!(outcome.status(), ScenarioStatus::Skipped);
    assert_eq!(
        outcome.steps().len(),
        3,
        "the trailing invocation is bypassed, not dropped",
    );

    let all = seen(&captured);
    let steps = per_step_events(&all);
    assert_eq!(
        steps.len(),
        3,
        "one per-step event per recorded invocation; captured {all:?}",
    );
    // The whole point of reading values: the statuses must *follow the plan*,
    // in order. A runner that emitted the same status three times, or that
    // numbered them from one instead of zero, passes every name-only assertion.
    let observed: Vec<(&str, &str)> = steps
        .iter()
        .map(|step| (value(&step.values, "index"), value(&step.values, "status")))
        .collect();
    assert_eq!(
        observed,
        vec![("0", "Passed"), ("1", "Skipped"), ("2", "Bypassed")],
        "the three statuses in plan order, each at its own index",
    );
    // The keyword rides along with each event, and the second invocation is the
    // `Given` that skipped, so a transposition of the three events' keywords
    // would show here even though the statuses would not.
    for (step, expected) in steps.iter().zip(["Given", "Given", "Then"]) {
        assert_eq!(
            value(&step.values, "keyword"),
            expected,
            "each event names the keyword of the invocation it describes",
        );
    }
}

/// The terminal skip is warned once, with its location and the message's
/// *presence*.
///
/// D14 requires index, `path:line`, and — for a skip — whether it carried a
/// reason, never the reason itself, which is step-supplied text of unbounded
/// length. A permitted skip is not a failure, so it is logged at `WARN` for the
/// same reason as a failure: the run stopped early, and a reader needs to know
/// where. The location is asserted by name because its absence is a real
/// regression rather than a cosmetic one: a warning with no source is the one a
/// reader has to guess about.
#[test]
fn the_terminal_skip_is_warned_with_its_location() {
    let (captured, _guard) = capture(Level::WARN);
    let mut ctx = StepContext::default();
    let plan = plan(&[
        (StepKeyword::Given, "an instrumented step passes", 43),
        (StepKeyword::Given, "an instrumented step skips", 44),
    ]);

    let outcome = run_scenario(&plan, ScenarioScope::new(&mut ctx));
    assert_eq!(outcome.status(), ScenarioStatus::Skipped);

    let all = seen(&captured);
    let warning = carrying(&all, "location");
    assert_eq!(warning.level, Level::WARN, "D14 puts a terminal at WARN");
    // The skip is the second invocation, so its index is 1 — the terminal is the
    // invocation that *asked* to skip, not the one that got bypassed. The
    // location is the `path:line` rendering D14 specifies, which the plan built
    // from the step's own line (44) rather than the scenario's (42).
    assert_eq!(value(&warning.values, "index"), "1");
    assert_eq!(
        value(&warning.values, "location"),
        "notes/instrumented.md:44"
    );
    // The skip called `skip!("instrumentation wanted a skip")`, so a message was
    // present. D14 logs only its presence: the reason is step-supplied text of
    // unbounded length, so its content must not appear in the event.
    assert_eq!(value(&warning.values, "has_message"), "true");
    // The `message` field present here is *not* the step's reason — it is the
    // event's own format string, which `tracing` records like any other field.
    // So absence of a `message` field proves nothing, and the assertion has to
    // be that the reason's text appears nowhere in the captured values. Without
    // that, a runner that logged the reason under some other field name would
    // pass. `skip!` uses a distinctive literal so this search cannot collide
    // with the event's own text.
    for (field, captured) in &warning.values {
        assert!(
            !captured.contains("instrumentation wanted a skip"),
            "the skip's own reason text must never reach the log; field `{field}` carried \
             `{captured}`, and the full capture was {:?}",
            warning.values,
        );
    }
}

/// The terminal failure is warned once, with its location and its error kind.
///
/// The step here names nothing registered, so the run fails the way
/// `runner_wire.rs` documents. D14 requires the error's *kind* discriminant and
/// never the formatted message, which is unbounded localized text.
#[test]
fn the_terminal_failure_is_warned_with_its_location_and_kind() {
    let (captured, _guard) = capture(Level::WARN);
    let mut ctx = StepContext::default();
    let plan = plan(&[(StepKeyword::Given, "a step nobody wrote", 43)]);

    let outcome = run_scenario(&plan, ScenarioScope::new(&mut ctx));
    assert_eq!(
        outcome.status(),
        ScenarioStatus::Failed,
        "the invocation must reach the registry and fail there",
    );

    let all = seen(&captured);
    let warning = carrying(&all, "kind");
    assert_eq!(warning.level, Level::WARN, "D14 puts a terminal at WARN");
    // The unregistered step is the only invocation, and it failed at the
    // registry rather than reaching a handler, so the index is 0 and the
    // classification is `Undefined` — the variant `FailureKind::of` maps an
    // unresolvable pattern to. `kind` is logged with `?`, so it renders as the
    // bare variant name.
    assert_eq!(value(&warning.values, "index"), "0");
    assert_eq!(
        value(&warning.values, "location"),
        "notes/instrumented.md:43"
    );
    assert_eq!(value(&warning.values, "kind"), "Undefined");
}

/// A `WARN` filter admits the terminal warning and none of the lighter events.
///
/// This is the non-vacuity control for the file. Without it, every assertion
/// above would also hold against a subscriber that recorded everything it was
/// handed regardless of level — which would mean the four events were not gated
/// at all, and that a production subscriber running at `WARN` would still pay
/// for the span and the per-step traces. The two halves are asserted together
/// so neither can pass alone: the warning still arrives, and nothing lighter
/// does.
#[test]
fn a_warn_filter_admits_the_terminal_warning_and_nothing_else() {
    let (captured, _guard) = capture(Level::WARN);
    let mut ctx = StepContext::default();
    let plan = plan(&[(StepKeyword::Given, "a step nobody wrote", 43)]);

    let _outcome = run_scenario(&plan, ScenarioScope::new(&mut ctx));

    let all: Vec<Captured> = seen(&captured);
    assert!(
        all.iter().all(|item| item.level == Level::WARN),
        "a WARN filter must not admit the span or the per-step traces; captured {all:?}",
    );
    assert!(
        !all.is_empty(),
        "the terminal failure is a WARN, so the filter must still admit it",
    );
}

/// The asynchronous driver attributes its events to the scenario span.
///
/// The async driver attaches the span with `Instrument` rather than entering it
/// with a guard, because a guard held across `.await` is thread-local and would
/// report this scenario as current on a thread that had moved on (AGENTS.md
/// forbids the form outright). Every test above drives the *synchronous* runner,
/// so none of them would notice the span being dropped from this path.
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

    // The assertion that carries the `Instrument` decision: the policy event is
    // emitted inside the instrumented future, and this would be missing if the
    // driver opened the span without keeping it current across the poll.
    let all = seen(&captured);
    let resolved = carrying(&all, "forced_failure");
    assert_eq!(value(&resolved.values, "plan_allows_skipping"), "false");
}
