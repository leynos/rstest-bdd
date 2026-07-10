//! Unit tests for registry collection and parsing.

use super::*;
use cargo_metadata::Message as MetadataMessage;
use rstest::rstest;

#[test]
fn detects_unrecognised_flag_from_libtest_getopts() {
    let stderr = "error: Unrecognized option: 'dump-steps'\n";
    assert!(is_unrecognised_dump_steps(stderr));
}

#[test]
fn detects_unrecognised_flag_from_clap() {
    let stderr = "error: Found argument '--dump-steps' which wasn't expected\n";
    assert!(is_unrecognised_dump_steps(stderr));
}

#[test]
fn ignores_unrelated_failures_containing_dump_steps() {
    let stderr = concat!(
        "test failed: invalid option for upstream tool\n",
        "hint: rerun with --dump-steps for diagnostics\n"
    );
    assert!(!is_unrecognised_dump_steps(stderr));
}

#[test]
#[expect(
    clippy::expect_used,
    reason = "Test should fail fast when the registry dump JSON is invalid."
)]
fn parses_registry_dump_with_bypassed_steps() {
    let json = r#"
    {
      "steps": [{"keyword":"Given","pattern":"x","file":"f","line":1,"used":false}],
      "scenarios": [{
        "feature_path":"feature",
        "scenario_name":"scenario",
        "status":"skipped",
        "message":"reason",
        "allow_skipped":true,
        "forced_failure":false,
        "line":42,
        "tags":["@t"]
      }],
      "bypassed_steps": [{
        "keyword":"Given",
        "pattern":"x",
        "file":"f",
        "line":1,
        "feature_path":"feature",
        "scenario_name":"scenario",
        "scenario_line":42,
        "tags":["@t"],
        "reason":"reason"
      }]
    }
    "#;
    let parsed = parse_registry_dump(json.as_bytes()).expect("valid dump");
    let Some(scenario) = parsed.scenarios.first() else {
        panic!("scenario entry");
    };
    assert_eq!(scenario.line, 42);
    assert_eq!(scenario.tags, vec!["@t".to_string()]);
    let Some(bypassed) = parsed.bypassed_steps.first() else {
        panic!("bypassed entry");
    };
    assert_eq!(bypassed.scenario_line, 42);
    assert_eq!(bypassed.tags, vec!["@t".to_string()]);
    assert_eq!(bypassed.reason.as_deref(), Some("reason"));
}

fn parse_message(json: &str) -> serde_json::Result<MetadataMessage> {
    serde_json::from_str(json)
}

/// Assert that `extract_test_executable` maps a cargo JSON message to the
/// expected executable path.
///
/// A macro rather than a helper function so that panic line numbers point at
/// the calling test.
macro_rules! assert_extracts_executable {
    ($json:expr, $expected:expr) => {{
        let msg = match parse_message($json) {
            Ok(msg) => msg,
            Err(err) => panic!("message should parse: {err}"),
        };
        assert_eq!(extract_test_executable(&msg), $expected);
    }};
}

#[rstest]
#[case::filters_non_tests(
    r#"{
            "reason": "compiler-artifact",
            "package_id": "pkg 0.1.0 (path+file:///tmp/pkg)",
            "target": {
                "kind": ["bin"],
                "crate_types": ["bin"],
                "name": "pkg",
                "src_path": "/tmp/src/main.rs",
                "edition": "2021",
                "doc": false,
                "doctest": false,
                "test": false
            },
            "profile": {
                "opt_level": "0",
                "debuginfo": 0,
                "debug_assertions": false,
                "overflow_checks": false,
                "test": false
            },
            "features": [],
            "filenames": ["/tmp/bin"],
            "executable": "/tmp/bin",
            "fresh": true
        }"#,
    None
)]
#[case::accepts_tests(
    r#"{
            "reason": "compiler-artifact",
            "package_id": "pkg 0.1.0 (path+file:///tmp/pkg)",
            "target": {
                "kind": ["test"],
                "crate_types": ["test"],
                "name": "pkg",
                "src_path": "/tmp/src/main.rs",
                "edition": "2021",
                "doc": false,
                "doctest": false,
                "test": true
            },
            "profile": {
                "opt_level": "0",
                "debuginfo": 0,
                "debug_assertions": false,
                "overflow_checks": false,
                "test": true
            },
            "features": [],
            "filenames": ["/tmp/test-bin"],
            "executable": "/tmp/test-bin",
            "fresh": true
        }"#,
    Some(PathBuf::from("/tmp/test-bin"))
)]
fn extract_test_executable_behaviour(#[case] json: &str, #[case] expected: Option<PathBuf>) {
    assert_extracts_executable!(json, expected);
}
