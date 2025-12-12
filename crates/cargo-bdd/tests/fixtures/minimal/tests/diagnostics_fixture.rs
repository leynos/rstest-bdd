//! Fixture seeding reporting diagnostics for snapshot-based integration tests.

use rstest_bdd as bdd;
use rstest_bdd::{step, StepContext, StepExecution, StepKeyword};
use serial_test::serial;

step!(StepKeyword::Given, "fixture bypassed step", bypassed_step, &[]);
step!(StepKeyword::Then, "fixture forced bypass", forced_bypass, &[]);

fn bypassed_step(
    _ctx: &mut StepContext<'_>,
    _text: &str,
    _docstring: Option<&str>,
    _table: Option<&[&[&str]]>,
) -> Result<StepExecution, bdd::StepError> {
    Ok(StepExecution::Continue { value: None })
}

fn forced_bypass(
    _ctx: &mut StepContext<'_>,
    _text: &str,
    _docstring: Option<&str>,
    _table: Option<&[&[&str]]>,
) -> Result<StepExecution, bdd::StepError> {
    Ok(StepExecution::Continue { value: None })
}

fn seed_reporting_fixture() {
    let metadata = bdd::reporting::ScenarioMetadata::new(
        "tests/features/diagnostics.fixture",
        "fixture skipped scenario",
        7,
        vec!["@allow_skipped".into()],
    );
    bdd::reporting::record(bdd::reporting::ScenarioRecord::from_metadata(
        metadata,
        bdd::reporting::ScenarioStatus::Skipped(bdd::reporting::SkippedScenario::new(
            Some("fixture skip message".into()),
            true,
            false,
        )),
    ));

    bdd::record_bypassed_steps(
        "tests/features/diagnostics.fixture",
        "fixture skipped scenario",
        7,
        vec!["@allow_skipped".into()],
        Some("fixture skip message"),
        [(StepKeyword::Given, "fixture bypassed step")],
    );

    let forced_metadata = bdd::reporting::ScenarioMetadata::new(
        "tests/features/diagnostics.fixture",
        "fixture forced failure skip",
        12,
        vec!["@critical".into()],
    );
    bdd::reporting::record(bdd::reporting::ScenarioRecord::from_metadata(
        forced_metadata,
        bdd::reporting::ScenarioStatus::Skipped(bdd::reporting::SkippedScenario::new(
            Some("fixture forced skip".into()),
            false,
            true,
        )),
    ));

    bdd::record_bypassed_steps(
        "tests/features/diagnostics.fixture",
        "fixture forced failure skip",
        12,
        vec!["@critical".into()],
        Some("fixture forced skip"),
        [(StepKeyword::Then, "fixture forced bypass")],
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
