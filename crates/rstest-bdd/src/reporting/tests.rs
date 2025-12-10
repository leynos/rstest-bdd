//! Unit tests for the reporting module.

use super::*;
use serial_test::serial;

#[test]
#[serial]
fn drain_clears_records() {
    let _ = drain();
    record(ScenarioRecord::new(
        "feature",
        "scenario",
        1,
        Vec::new(),
        ScenarioStatus::Passed,
    ));
    assert_eq!(snapshot().len(), 1);
    let drained = drain();
    assert_eq!(drained.len(), 1);
    assert!(snapshot().is_empty());
}

#[test]
#[serial]
fn skipped_records_store_metadata() {
    let _ = drain();
    let details = SkippedScenario::new(Some("pending".into()), true, false);
    record(ScenarioRecord::new(
        "feature",
        "scenario",
        2,
        vec!["@allow_skipped".into()],
        ScenarioStatus::Skipped(details.clone()),
    ));
    let records = drain();
    assert_eq!(records.len(), 1);
    let Some(record) = records.first() else {
        panic!("collector should retain the recorded skip");
    };
    match record.status() {
        ScenarioStatus::Skipped(stored) => {
            assert_eq!(stored.message(), Some("pending"));
            assert!(stored.allow_skipped());
            assert!(!stored.forced_failure());
        }
        ScenarioStatus::Passed => panic!("expected skipped record"),
    }
}
