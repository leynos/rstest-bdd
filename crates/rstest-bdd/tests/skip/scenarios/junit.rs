//! `JUnit` diagnostic-report assertions for skipped scenarios.

#![cfg(feature = "diagnostics")]

use rstest_bdd::reporting::{
    self as reporting,
    ScenarioMetadata,
    ScenarioRecord,
    ScenarioStatus,
    SkippedScenario,
    drain as drain_reports,
    record as record_scenario,
};
use serial_test::serial;

use super::{FailOnSkippedGuard, allowed_skip, disallowed_skip};

#[test]
#[serial(skip_reporting)]
fn junit_writer_emits_skipped_child_element() {
    let _ = drain_reports();
    let guard = FailOnSkippedGuard::enable();
    allowed_skip();
    drop(guard);
    let records = reporting::snapshot();
    let mut output = String::new();
    if let Err(error) = reporting::junit::write(&mut output, &records) {
        panic!("expected to render JUnit report: {error}");
    }
    assert!(
        output.contains("<skipped message=\"skip requested for coverage\" />"),
        "JUnit output should include skipped element with message",
    );
    assert!(
        output.contains("tests=\"1\" failures=\"0\" skipped=\"1\""),
        "JUnit suite summary should record skip counts",
    );
    let _ = drain_reports();
}

#[test]
#[serial(skip_reporting)]
fn junit_writer_marks_forced_failure_skips() {
    let _ = drain_reports();
    let guard = FailOnSkippedGuard::enable();
    let _ = std::panic::catch_unwind(disallowed_skip);
    drop(guard);
    let records = reporting::snapshot();
    let mut output = String::new();
    if let Err(error) = reporting::junit::write(&mut output, &records) {
        panic!("expected to render JUnit report: {error}");
    }
    assert!(
        output.contains("<failure type=\"fail_on_skipped\">")
            && output.contains("fail_on_skipped enabled"),
        "forced failure skip should surface as failure in JUnit",
    );
    assert!(
        output.contains("failures=\"1\" skipped=\"1\""),
        "JUnit summary should reflect failure counts",
    );
    let _ = drain_reports();
}

#[test]
#[serial(skip_reporting)]
fn junit_writer_escapes_special_characters() {
    let _ = drain_reports();
    let metadata = ScenarioMetadata::new(
        "tests/features/<feature>&special",
        "Scenario with <&>\"'",
        1,
        Vec::new(),
    );
    record_scenario(ScenarioRecord::from_metadata(
        metadata,
        ScenarioStatus::Skipped(SkippedScenario::new(
            Some("message with <bad>&chars\u{0007}".into()),
            true,
            false,
        )),
    ));
    let records = reporting::snapshot();
    let mut output = String::new();
    if let Err(error) = reporting::junit::write(&mut output, &records) {
        panic!("expected to render JUnit report: {error}");
    }
    assert!(output.contains("Scenario with &lt;&amp;&gt;&quot;&apos;"));
    assert!(output.contains("tests/features/&lt;feature&gt;&amp;special"));
    assert!(output.contains("message with &lt;bad&gt;&amp;chars"));
    assert!(output.contains("&#xFFFD;"));
    let _ = drain_reports();
}
