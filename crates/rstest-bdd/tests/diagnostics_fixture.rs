//! Diagnostics fixture to expose skip reporting to cargo-bdd.
//!
//! When the diagnostics runner (`cargo bdd`) requests a registry dump the
//! environment variable `RSTEST_BDD_DUMP_STEPS` is present. We synthesise a
//! couple of scenario outcomes in that mode so the CLI can exercise the
//! reporting pipeline without running a full behaviour suite.

use rstest_bdd as bdd;

#[cfg(feature = "diagnostics")]
fn seed_reporting_fixture() {
    if std::env::var_os("RSTEST_BDD_DUMP_STEPS").is_none() {
        return;
    }

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

#[cfg(feature = "diagnostics")]
inventory::submit! {
    bdd::reporting::DumpSeed::new(seed_reporting_fixture)
}

#[test]
fn diagnostics_fixture_runs() {
    #[cfg(feature = "diagnostics")]
    {
        let _ = bdd::reporting::drain();
        std::env::set_var("RSTEST_BDD_DUMP_STEPS", "1");
        bdd::reporting::run_dump_seeds();
        let snapshot = bdd::reporting::snapshot();
        assert!(snapshot
            .iter()
            .any(|record| matches!(record.status(), bdd::reporting::ScenarioStatus::Passed)));
        assert!(snapshot.iter().any(|record| {
            matches!(record.status(), bdd::reporting::ScenarioStatus::Skipped(details) if details
                .forced_failure())
        }));
        assert!(snapshot.iter().any(|record| {
            matches!(record.status(), bdd::reporting::ScenarioStatus::Skipped(details) if !details
                .forced_failure())
        }));
        std::env::remove_var("RSTEST_BDD_DUMP_STEPS");
        let _ = bdd::reporting::drain();
    }

    #[cfg(not(feature = "diagnostics"))]
    {
        // Without the diagnostics feature the snapshot remains accessible and
        // ensures compilation still succeeds.
        let _ = bdd::reporting::snapshot();
    }
}
