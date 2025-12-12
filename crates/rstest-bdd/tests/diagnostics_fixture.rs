//! Diagnostics fixture to expose skip reporting to cargo-bdd.
//!
//! When the diagnostics runner (`cargo bdd`) requests a registry dump the
//! environment variable `RSTEST_BDD_DUMP_STEPS` is present. We synthesise a
//! couple of scenario outcomes in that mode so the CLI can exercise the
//! reporting pipeline without running a full behaviour suite.

use rstest_bdd as bdd;
#[cfg(feature = "diagnostics")]
use serial_test::serial;
#[cfg(feature = "diagnostics")]
use std::sync::atomic::{AtomicBool, Ordering};

#[cfg(feature = "diagnostics")]
static SHOULD_SEED: AtomicBool = AtomicBool::new(false);

#[cfg(feature = "diagnostics")]
fn seed_reporting_fixture() {
    if !should_seed_dump_steps() {
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

#[cfg(feature = "diagnostics")]
struct DumpStepsGuard;

#[cfg(feature = "diagnostics")]
impl DumpStepsGuard {
    fn set() -> Self {
        SHOULD_SEED.store(true, Ordering::SeqCst);
        Self
    }
}

#[cfg(feature = "diagnostics")]
impl Drop for DumpStepsGuard {
    fn drop(&mut self) {
        SHOULD_SEED.store(false, Ordering::SeqCst);
    }
}

#[cfg(feature = "diagnostics")]
fn should_seed_dump_steps() -> bool {
    if SHOULD_SEED.load(Ordering::SeqCst) {
        return true;
    }

    std::env::var_os("RSTEST_BDD_DUMP_STEPS").is_some()
}

#[test]
#[cfg_attr(feature = "diagnostics", serial)]
fn diagnostics_fixture_runs() {
    #[cfg(feature = "diagnostics")]
    {
        let _ = bdd::reporting::drain();
        let _guard = DumpStepsGuard::set();
        bdd::reporting::run_dump_seeds();
        let snapshot = bdd::reporting::snapshot();
        assert!(
            snapshot
                .iter()
                .any(|record| matches!(record.status(), bdd::reporting::ScenarioStatus::Passed))
        );
        assert!(snapshot.iter().any(|record| {
            matches!(record.status(), bdd::reporting::ScenarioStatus::Skipped(details) if details
                .forced_failure())
        }));
        assert!(snapshot.iter().any(|record| {
            matches!(record.status(), bdd::reporting::ScenarioStatus::Skipped(details) if !details
                .forced_failure())
        }));
        let _ = bdd::reporting::drain();
    }

    #[cfg(not(feature = "diagnostics"))]
    {
        // Without the diagnostics feature the snapshot remains accessible and
        // ensures compilation still succeeds.
        let _ = bdd::reporting::snapshot();
    }
}
