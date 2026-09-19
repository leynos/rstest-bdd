//! D14: the four `tracing` events the runner is specified to emit.
//!
//! D14 names four events and says what each must carry. Naming them is not
//! emitting them, and an event whose fields are never read is indistinguishable
//! from an event that was never written: `make test` would pass either way, and
//! so would a subscriber in production that filtered on a misspelled field
//! name. This file captures the events and asserts on their fields.
//!
//! The capture machinery — a hand-rolled `tracing::Subscriber`, and the readers
//! that pull field names back out of it — lives in the companion `capture`
//! module, with the reasoning for that choice. What is here is the plan, the
//! registered steps, and the six assertions that are D14's obligations.
//!
//! # Why this is an integration test
//!
//! D21, as amended: any runner test that calls `run_scenario` or
//! `run_scenario_async` is an integration test. The first registry lookup in a
//! process builds `STEP_MAP`, whose duplicate-step `assert!` fires on the
//! pattern `registry/introspection.rs` registers twice on purpose, so the
//! unit-test binary cannot reach the registry *at all* — with or without the
//! runner — and a run there can only ever terminate as an unresolvable
//! failure. This file was first written as a unit module on the narrower
//! reading that a non-resolving test escapes the rule; all six panicked.
//! `runner_wire.rs` reached the same conclusion for the same reason.

use rstest_bdd::{
    StepContext,
    StepKeyword,
    runner::{ScenarioPlan, ScenarioPlanBuilder, ScenarioScope, ScenarioStatus, run_scenario},
};
use rstest_bdd_macros::given;
use tracing::Level;

#[path = "runner_instrumentation/capture.rs"]
mod capture;

use capture::{Captured, capture, carrying, per_step_events, scenario_span_fields, seen};

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
/// count, and `allow_skipped`. All five are asserted, because a span missing
/// one of them is still opened and still named `scenario`, so an assertion that
/// only checked for the span's existence would not notice.
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

    let fields = scenario_span_fields(&captured);
    for field in ["name", "source", "line", "steps", "allow_skipped"] {
        assert!(
            fields.contains(field),
            "the scenario span must carry `{field}`; it carried {fields:?}",
        );
    }
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
    for field in [
        "plan_allows_skipping",
        "fail_on_skipped",
        "allow_skipped",
        "forced_failure",
    ] {
        assert!(
            resolved.fields.contains(field),
            "the resolution event must carry `{field}`; it carried {:?}",
            resolved.fields,
        );
    }
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
    for step in steps {
        for field in ["index", "keyword", "status"] {
            assert!(
                step.fields.contains(field),
                "the per-step event must carry `{field}`; it carried {:?}",
                step.fields,
            );
        }
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
    for field in ["index", "location", "has_message"] {
        assert!(
            warning.fields.contains(field),
            "the terminal skip warning must carry `{field}`; it carried {:?}",
            warning.fields,
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
    for field in ["index", "location", "kind"] {
        assert!(
            warning.fields.contains(field),
            "the terminal failure warning must carry `{field}`; it carried {:?}",
            warning.fields,
        );
    }
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
