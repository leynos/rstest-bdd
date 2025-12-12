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
use rstest_bdd::{step, StepContext, StepExecution, StepKeyword};

#[cfg(feature = "diagnostics")]
step!(
    StepKeyword::Given,
    "fixture bypassed step",
    bypassed_step,
    &[]
);

#[cfg(feature = "diagnostics")]
step!(
    StepKeyword::Then,
    "fixture forced bypass",
    forced_bypass,
    &[]
);

#[cfg(feature = "diagnostics")]
#[allow(clippy::unnecessary_wraps)]
fn bypassed_step(
    _ctx: &mut StepContext<'_>,
    _text: &str,
    _docstring: Option<&str>,
    _table: Option<&[&[&str]]>,
) -> Result<StepExecution, bdd::StepError> {
    Ok(StepExecution::Continue { value: None })
}

#[cfg(feature = "diagnostics")]
#[allow(clippy::unnecessary_wraps)]
fn forced_bypass(
    _ctx: &mut StepContext<'_>,
    _text: &str,
    _docstring: Option<&str>,
    _table: Option<&[&[&str]]>,
) -> Result<StepExecution, bdd::StepError> {
    Ok(StepExecution::Continue { value: None })
}

#[cfg(feature = "diagnostics")]
static SHOULD_SEED: AtomicBool = AtomicBool::new(false);

#[cfg(feature = "diagnostics")]
fn seed_reporting_fixture() {
    if !should_seed_dump_steps() {
        return;
    }

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
        let has_passed = snapshot
            .iter()
            .any(|record| matches!(record.status(), bdd::reporting::ScenarioStatus::Passed));
        let has_forced_skip = snapshot.iter().any(|record| {
            matches!(
                record.status(),
                bdd::reporting::ScenarioStatus::Skipped(details) if details.forced_failure()
            )
        });
        let has_non_forced_skip = snapshot.iter().any(|record| {
            matches!(
                record.status(),
                bdd::reporting::ScenarioStatus::Skipped(details) if !details.forced_failure()
            )
        });

        assert!(
            has_passed,
            "expected at least one Passed scenario in diagnostics snapshot",
        );
        assert!(
            has_forced_skip,
            "expected at least one forced-failure Skipped scenario in diagnostics snapshot",
        );
        assert!(
            has_non_forced_skip,
            "expected at least one non-forced Skipped scenario in diagnostics snapshot",
        );
        let _ = bdd::reporting::drain();
    }

    #[cfg(not(feature = "diagnostics"))]
    {
        // Without the diagnostics feature the snapshot remains accessible and
        // ensures compilation still succeeds.
        let _ = bdd::reporting::snapshot();
    }
}
