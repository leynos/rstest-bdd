//! Unit tests for the D5 conversion.
//!
//! Every case here is non-vacuous in the same way: the conversion could return
//! the wrong record, or the wrong `Gap`, and each test names which. The one
//! `Gap` arm the engine's own assembly makes unreachable is built by hand, so
//! the test observes the branch firing rather than assuming it does.

use super::{Gap, record_from};
use crate::{
    StepKeyword,
    execution::ExecutionError,
    reporting::ScenarioStatus,
    runner::{
        ScenarioFailure,
        ScenarioOutcome,
        ScenarioPlan,
        ScenarioPlanBuilder,
        ScenarioSkip,
        ScenarioStatus as RunnerStatus,
        SkipPolicyRecord,
        SkipRecord,
        StepOutcome,
        ValueFate,
        test_invocation,
    },
};

/// A two-step plan with a line, a tag, and a source path.
///
/// Infallible: the builder takes owned literals and cannot fail, so this needs
/// no `#[test]`-only escape hatch.
fn plan() -> ScenarioPlan {
    ScenarioPlanBuilder::new("Add two numbers", "notes/arithmetic.md")
        .at_line(42)
        .tag("@allow_skipped")
        .step_at(StepKeyword::Given, "a calculator", 43)
        .step_at(StepKeyword::Then, "the result is 4", 45)
        .build()
}

/// The first step, passed and returning nothing.
fn first_step() -> StepOutcome {
    StepOutcome::passed(
        0,
        &test_invocation(StepKeyword::Given, "a calculator", None),
        None,
    )
}

/// A step-not-found error naming the second invocation.
fn not_found() -> ExecutionError {
    ExecutionError::StepNotFound {
        index: 1,
        keyword: StepKeyword::Then,
        text: "the result is 4".into(),
        feature_path: "notes/arithmetic.md".into(),
        scenario_name: "Add two numbers".into(),
    }
}

/// A plan and a passing outcome convert into a record carrying both.
///
/// This is the smoke test D5 asks for: the claim that the two models are
/// reconcilable at all. It fails if any of the four plan-side fields stops
/// reaching the record, which is the loss the conversion exists to prevent — a
/// record that dropped its scenario name still records, and nothing downstream
/// would notice.
#[test]
fn a_passing_run_converts() {
    let plan = plan();
    let outcome = ScenarioOutcome::new(
        RunnerStatus::Passed,
        vec![
            first_step(),
            StepOutcome::passed(
                1,
                &test_invocation(StepKeyword::Then, "the result is 4", None),
                None,
            ),
        ],
        None,
        None,
    );

    let record = record_from(&plan, &outcome).expect("a passing run has a representation");

    assert_eq!(record.scenario_name(), "Add two numbers");
    assert_eq!(record.feature_path(), "notes/arithmetic.md");
    assert_eq!(record.line(), 42, "the plan's line must reach the record");
    assert_eq!(
        record.tags(),
        &["@allow_skipped".to_owned()],
        "the plan's tags must reach the record; the outcome does not carry them",
    );
    assert_eq!(
        record.status(),
        &ScenarioStatus::Passed,
        "a passing run must record as passed",
    );
}

/// A skipped run converts, and its resolved skip policy survives the crossing.
///
/// The three `reporting::SkippedScenario` fields are asserted together because they are
/// one decision: the runner resolves `allow_skipped` and `forced_failure` once,
/// and a conversion that recomputed either from the other would be free to
/// disagree with the outcome it was handed.
#[test]
fn a_skipped_run_converts_with_its_policy() {
    let plan = plan();
    let skip = ScenarioSkip::new(
        1,
        SkipRecord {
            message: Some("pending".into()),
            source: None,
        },
        SkipPolicyRecord {
            allow_skipped: false,
            forced_failure: true,
        },
    );
    let outcome = ScenarioOutcome::new(
        RunnerStatus::Skipped,
        vec![
            first_step(),
            StepOutcome::skipped(
                1,
                &test_invocation(StepKeyword::Then, "the result is 4", None),
                Some("pending".into()),
            ),
        ],
        Some(skip),
        None,
    );

    let record = record_from(&plan, &outcome).expect("a skipped run has a representation");

    let ScenarioStatus::Skipped(stored) = record.status() else {
        panic!(
            "a skipped run must record as skipped; got {:?}",
            record.status()
        );
    };
    assert_eq!(stored.message(), Some("pending"));
    assert!(
        !stored.allow_skipped(),
        "the scenario did not permit skipping, and the record must not say it did",
    );
    assert!(
        stored.forced_failure(),
        "policy forced this skip to fail, and the record must carry that",
    );
}

/// A failed run reports [`Gap::Failure`], not a passing record.
///
/// This is D5's central finding, and this test is its evidence:
/// `reporting::ScenarioStatus` has two variants and neither is a failure, so
/// there is no correct record to build. Degrading to `Passed` would report a
/// crashed scenario as green — the false green the whole runner exists to
/// avoid — so the conversion refuses instead.
///
/// The refusal is also what makes the gap non-vacuous. Once 13.2.1 adds the
/// variant, this test stops compiling rather than quietly continuing to pass,
/// which is the prompt to place the failure in the record and delete the arm.
#[test]
fn a_failed_run_reports_the_missing_failure_case() {
    let plan = plan();
    let outcome = ScenarioOutcome::new(
        RunnerStatus::Failed,
        vec![
            first_step(),
            StepOutcome::failed(
                1,
                &test_invocation(StepKeyword::Then, "the result is 4", None),
                not_found(),
            ),
        ],
        None,
        Some(ScenarioFailure::Step {
            index: 1,
            error: not_found(),
        }),
    );

    assert_eq!(
        record_from(&plan, &outcome),
        Err(Gap::Failure),
        "a failed run has no reporting representation, and the conversion must say so",
    );
}

/// A plan with no line reports [`Gap::MissingLine`] rather than inventing one.
///
/// `ScenarioMetadata::line` is a non-optional `u32`, so a frontend that does
/// not track scenario lines — which D3 permits, since the source identity is
/// opaque — has no record at all. Substituting `0` would write a coordinate the
/// frontend never observed into a reportable artefact; `SourceLocation` already
/// rejects zero as a line for exactly that reason.
#[test]
fn a_plan_without_a_line_reports_the_gap() {
    let plan = ScenarioPlanBuilder::new("Add two numbers", "notes/arithmetic.md")
        .step_at(StepKeyword::Given, "a calculator", 43)
        .build();
    assert_eq!(
        plan.source_line(),
        None,
        "the builder must not invent a line"
    );
    let outcome = ScenarioOutcome::new(RunnerStatus::Passed, vec![first_step()], None, None);

    assert_eq!(record_from(&plan, &outcome), Err(Gap::MissingLine));
}

/// A skipped status with no skip record reports the gap rather than panicking.
///
/// The engine cannot build this state: `assemble` produces `Skipped` and the
/// skip record from one match arm. So it is constructed by hand here, and that
/// is the point — a branch whose firing has never been observed is
/// indistinguishable from no branch at all, and this one's entire justification
/// is that it is reachable in principle.
#[test]
fn a_skip_without_a_record_reports_the_gap() {
    let plan = plan();
    let outcome = ScenarioOutcome::new(
        RunnerStatus::Skipped,
        vec![StepOutcome::skipped(
            0,
            &test_invocation(StepKeyword::Given, "a calculator", None),
            None,
        )],
        None,
        None,
    );

    assert_eq!(
        record_from(&plan, &outcome),
        Err(Gap::MissingSkipRecord),
        "a skip with no record must be reported, not assumed away",
    );
}

/// A passed step's value fate does not reach the record, deliberately.
///
/// `reporting` has no vocabulary for [`ValueFate`], and nothing downstream
/// branches on it: the runtime already warns for `AmbiguousIgnored` at
/// insertion time, and `NoMatch` is a runner-side diagnostic about a value
/// reaching no later step. Recorded as a test rather than as prose so a future
/// reader sees the omission was considered; if 13.2.1 does need it, it will
/// find the requirement here rather than at migration time.
#[test]
fn a_value_fate_does_not_reach_the_record() {
    let plan = plan();
    let outcome = ScenarioOutcome::new(
        RunnerStatus::Passed,
        vec![StepOutcome::passed(
            0,
            &test_invocation(StepKeyword::Given, "a calculator", None),
            Some(ValueFate::AmbiguousIgnored),
        )],
        None,
        None,
    );

    let record = record_from(&plan, &outcome).expect("a passing run has a representation");
    assert_eq!(
        record.status(),
        &ScenarioStatus::Passed,
        "an ambiguous insertion is not a scenario failure, so the run still records as passed",
    );
}
