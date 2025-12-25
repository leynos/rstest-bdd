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
use rstest_bdd::{StepContext, StepExecution, StepFuture, StepKeyword, step};

#[cfg(feature = "diagnostics")]
step!(
    StepKeyword::Given,
    "fixture bypassed step",
    bypassed_step,
    bypassed_step_async,
    &[]
);

#[cfg(feature = "diagnostics")]
step!(
    StepKeyword::Then,
    "fixture forced bypass",
    forced_bypass,
    forced_bypass_async,
    &[]
);

#[cfg(feature = "diagnostics")]
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

#[cfg(feature = "diagnostics")]
fn bypassed_step_async<'a>(
    ctx: &'a mut StepContext<'a>,
    text: &str,
    docstring: Option<&str>,
    table: Option<&[&[&str]]>,
) -> StepFuture<'a> {
    Box::pin(std::future::ready(bypassed_step(
        ctx, text, docstring, table,
    )))
}

#[cfg(feature = "diagnostics")]
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

#[cfg(feature = "diagnostics")]
fn forced_bypass_async<'a>(
    ctx: &'a mut StepContext<'a>,
    text: &str,
    docstring: Option<&str>,
    table: Option<&[&[&str]]>,
) -> StepFuture<'a> {
    Box::pin(std::future::ready(forced_bypass(
        ctx, text, docstring, table,
    )))
}

#[cfg(feature = "diagnostics")]
static SHOULD_SEED: AtomicBool = AtomicBool::new(false);

#[cfg(feature = "diagnostics")]
fn record_skipped_with_bypass(
    scenario_name: &str,
    line: u32,
    tags: Vec<String>,
    message: &str,
    allow_skipped: bool,
    forced_failure: bool,
    bypassed_step: (StepKeyword, &str),
) {
    const FEATURE_PATH: &str = "tests/features/diagnostics.fixture";

    let metadata =
        bdd::reporting::ScenarioMetadata::new(FEATURE_PATH, scenario_name, line, tags.clone());
    bdd::reporting::record(bdd::reporting::ScenarioRecord::from_metadata(
        metadata,
        bdd::reporting::ScenarioStatus::Skipped(bdd::reporting::SkippedScenario::new(
            Some(message.into()),
            allow_skipped,
            forced_failure,
        )),
    ));

    bdd::record_bypassed_steps(
        FEATURE_PATH,
        scenario_name,
        line,
        tags,
        Some(message),
        [bypassed_step],
    );
}

#[cfg(feature = "diagnostics")]
fn seed_reporting_fixture() {
    if !should_seed_dump_steps() {
        return;
    }

    record_skipped_with_bypass(
        "fixture skipped scenario",
        7,
        vec!["@allow_skipped".into()],
        "fixture skip message",
        true,
        false,
        (StepKeyword::Given, "fixture bypassed step"),
    );

    record_skipped_with_bypass(
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

#[test]
#[cfg(feature = "diagnostics")]
#[serial]
fn diagnostics_fixture_records_bypassed_metadata() {
    let _ = bdd::reporting::drain();
    let _guard = DumpStepsGuard::set();
    bdd::reporting::run_dump_seeds();
    let json = bdd::dump_registry().unwrap_or_else(|e| panic!("dump registry: {e}"));
    let parsed: serde_json::Value =
        serde_json::from_str(&json).unwrap_or_else(|e| panic!("valid json: {e}"));
    let bypassed = parsed
        .get("bypassed_steps")
        .and_then(|v| v.as_array())
        .unwrap_or_else(|| panic!("bypassed array"));
    let entry = bypassed
        .iter()
        .find(|value| {
            value["pattern"] == "fixture bypassed step"
                && value["scenario_name"] == "fixture skipped scenario"
                && value["scenario_line"].as_u64() == Some(7)
        })
        .unwrap_or_else(|| panic!("bypassed entry"));
    assert_eq!(entry["feature_path"], "tests/features/diagnostics.fixture");
    assert_eq!(entry["scenario_name"], "fixture skipped scenario");
    assert_eq!(entry["scenario_line"].as_u64(), Some(7));
    assert_eq!(
        entry["tags"]
            .as_array()
            .and_then(|tags| tags.first())
            .and_then(|v| v.as_str()),
        Some("@allow_skipped")
    );
    assert_eq!(entry["reason"], "fixture skip message");
    let _ = bdd::reporting::drain();
}
