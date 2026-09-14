//! Skip collector assertions.

use rstest_bdd::{
    assert_scenario_skipped,
    reporting::{ScenarioStatus, drain as drain_reports},
};
use serial_test::serial;

use super::{
    FailOnSkippedGuard,
    allowed_skip,
    allowed_skip_without_message,
    assert_feature_path_suffix,
    disallowed_skip,
    scenario_passes_without_skip,
};

#[test]
#[serial(skip_reporting)]
fn collector_records_allowed_skip_metadata() {
    let _ = drain_reports();
    let guard = FailOnSkippedGuard::enable();
    allowed_skip();
    drop(guard);
    let records = drain_reports();
    let [record] = records.as_slice() else {
        panic!("expected a single skip record");
    };
    assert_feature_path_suffix(record.feature_path(), "tests/features/skip.feature");
    assert_eq!(record.scenario_name(), "allowed skip");
    let details = assert_scenario_skipped!(
        record.status(),
        message = "skip requested for coverage",
        allow_skipped = true,
        forced_failure = false,
    );
    assert_eq!(details.message(), Some("skip requested for coverage"));
}

#[test]
#[serial(skip_reporting)]
fn collector_marks_forced_failure_skips() {
    let _ = drain_reports();
    let guard = FailOnSkippedGuard::enable();
    let result = std::panic::catch_unwind(disallowed_skip);
    drop(guard);
    assert!(result.is_err(), "disallowed skip should panic");
    let records = drain_reports();
    let [record] = records.as_slice() else {
        panic!("expected a single skip record");
    };
    let details = assert_scenario_skipped!(
        record.status(),
        message = "skip requested for coverage",
        allow_skipped = false,
        forced_failure = true,
    );
    assert_eq!(details.message(), Some("skip requested for coverage"));
}

#[test]
#[serial(skip_reporting)]
fn collector_records_passed_scenarios() {
    let _ = drain_reports();
    scenario_passes_without_skip();
    let records = drain_reports();
    let [record] = records.as_slice() else {
        panic!("expected a single pass record");
    };
    assert_feature_path_suffix(record.feature_path(), "tests/features/reporting.feature");
    assert_eq!(record.scenario_name(), "scenario passes");
    assert!(matches!(record.status(), ScenarioStatus::Passed));
}

#[test]
#[serial(skip_reporting)]
fn collector_records_skips_without_message() {
    let _ = drain_reports();
    let guard = FailOnSkippedGuard::enable();
    allowed_skip_without_message();
    drop(guard);
    let records = drain_reports();
    let [record] = records.as_slice() else {
        panic!("expected a single skip record without message");
    };
    let details = assert_scenario_skipped!(
        record.status(),
        message_absent = true,
        allow_skipped = true,
        forced_failure = false,
    );
    assert_eq!(details.message(), None);
}
