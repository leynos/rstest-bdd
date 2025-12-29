//! Fixture seeding reporting diagnostics for snapshot-based integration tests.

use rstest_bdd as bdd;
use rstest_bdd::test_support::sync_to_async;
use rstest_bdd::{step, StepContext, StepExecution, StepFuture, StepKeyword};
use serial_test::serial;

step!(
    StepKeyword::Given,
    "fixture bypassed step",
    bypassed_step,
    bypassed_step_async,
    &[]
);
step!(
    StepKeyword::Then,
    "fixture forced bypass",
    forced_bypass,
    forced_bypass_async,
    &[]
);

#[expect(
    clippy::unnecessary_wraps,
    reason = "Step handlers must return Result<StepExecution, StepError>."
)]
fn bypassed_step(
    _ctx: &mut StepContext<'_>,
    _text: &str,
    _docstring: Option<&str>,
    _table: Option<&[&[&str]]>,
) -> Result<StepExecution, bdd::StepError> {
    Ok(StepExecution::Continue { value: None })
}

fn bypassed_step_async<'a>(
    ctx: &'a mut StepContext<'a>,
    text: &str,
    docstring: Option<&str>,
    table: Option<&[&[&str]]>,
) -> StepFuture<'a> {
    sync_to_async(bypassed_step)(ctx, text, docstring, table)
}

#[expect(
    clippy::unnecessary_wraps,
    reason = "Step handlers must return Result<StepExecution, StepError>."
)]
fn forced_bypass(
    _ctx: &mut StepContext<'_>,
    _text: &str,
    _docstring: Option<&str>,
    _table: Option<&[&[&str]]>,
) -> Result<StepExecution, bdd::StepError> {
    Ok(StepExecution::Continue { value: None })
}

fn forced_bypass_async<'a>(
    ctx: &'a mut StepContext<'a>,
    text: &str,
    docstring: Option<&str>,
    table: Option<&[&[&str]]>,
) -> StepFuture<'a> {
    sync_to_async(forced_bypass)(ctx, text, docstring, table)
}

fn record_skipped_with_bypass(
    feature_path: &str,
    scenario_name: &str,
    line: u32,
    tags: Vec<String>,
    message: &str,
    allow_skipped: bool,
    forced_failure: bool,
    bypassed_step: (StepKeyword, &str),
) {
    let metadata = bdd::reporting::ScenarioMetadata::new(
        feature_path,
        scenario_name,
        line,
        tags.clone(),
    );
    bdd::reporting::record(bdd::reporting::ScenarioRecord::from_metadata(
        metadata,
        bdd::reporting::ScenarioStatus::Skipped(bdd::reporting::SkippedScenario::new(
            Some(message.into()),
            allow_skipped,
            forced_failure,
        )),
    ));

    bdd::record_bypassed_steps(
        feature_path,
        scenario_name,
        line,
        tags,
        Some(message),
        [bypassed_step],
    );
}

fn seed_reporting_fixture() {
    record_skipped_with_bypass(
        "tests/features/diagnostics.fixture",
        "fixture skipped scenario",
        7,
        vec!["@allow_skipped".into()],
        "fixture skip message",
        true,
        false,
        (StepKeyword::Given, "fixture bypassed step"),
    );

    record_skipped_with_bypass(
        "tests/features/diagnostics.fixture",
        "fixture forced failure skip",
        12,
        vec!["@critical".into()],
        "fixture forced skip",
        false,
        true,
        (StepKeyword::Then, "fixture forced bypass"),
    );

    let passing_metadata = bdd::reporting::ScenarioMetadata::new(
        "tests/features/diagnostics.fixture",
        "fixture passing scenario",
        18,
        Vec::new(),
    );
    bdd::reporting::record(bdd::reporting::ScenarioRecord::from_metadata(
        passing_metadata,
        bdd::reporting::ScenarioStatus::Passed,
    ));
}

inventory::submit! {
    bdd::reporting::DumpSeed::new(seed_reporting_fixture)
}

#[test]
#[serial]
fn diagnostics_fixture_runs() {
    let _ = bdd::reporting::drain();
    seed_reporting_fixture();
    let snapshot = bdd::reporting::snapshot();
    assert!(snapshot.iter().any(|record| matches!(
        record.status(),
        bdd::reporting::ScenarioStatus::Passed
    )));
    assert!(snapshot.iter().any(|record| matches!(
        record.status(),
        bdd::reporting::ScenarioStatus::Skipped(details) if details.forced_failure()
    )));
    assert!(snapshot.iter().any(|record| matches!(
        record.status(),
        bdd::reporting::ScenarioStatus::Skipped(details) if !details.forced_failure()
    )));
    let _ = bdd::reporting::drain();
}
