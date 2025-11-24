use rstest_bdd as bdd;
use serial_test::serial;

fn seed_reporting_fixture() {
    bdd::reporting::record(bdd::reporting::ScenarioRecord::new(
        "tests/features/diagnostics.fixture",
        "fixture skipped scenario",
        bdd::reporting::ScenarioStatus::Skipped(bdd::reporting::SkippedScenario::new(
            Some("fixture skip message".into()),
            true,
            false,
        )),
    ));

    bdd::reporting::record(bdd::reporting::ScenarioRecord::new(
        "tests/features/diagnostics.fixture",
        "fixture forced failure skip",
        bdd::reporting::ScenarioStatus::Skipped(bdd::reporting::SkippedScenario::new(
            Some("fixture forced skip".into()),
            false,
            true,
        )),
    ));

    bdd::reporting::record(bdd::reporting::ScenarioRecord::new(
        "tests/features/diagnostics.fixture",
        "fixture passing scenario",
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
