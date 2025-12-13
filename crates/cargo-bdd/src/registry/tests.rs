//! Unit tests for registry collection and parsing.

use super::*;
use cargo_metadata::Message as MetadataMessage;

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
    let parsed =
        parse_registry_dump(json.as_bytes()).unwrap_or_else(|err| panic!("valid dump: {err}"));
    let scenario = parsed
        .scenarios
        .first()
        .unwrap_or_else(|| panic!("scenario entry"));
    assert_eq!(scenario.line, 42);
    assert_eq!(scenario.tags, vec!["@t".to_string()]);
    let bypassed = parsed
        .bypassed_steps
        .first()
        .unwrap_or_else(|| panic!("bypassed entry"));
    assert_eq!(bypassed.scenario_line, 42);
    assert_eq!(bypassed.tags, vec!["@t".to_string()]);
    assert_eq!(bypassed.reason.as_deref(), Some("reason"));
}

fn parse_message(json: &str) -> MetadataMessage {
    serde_json::from_str(json).unwrap_or_else(|err| panic!("message should parse: {err}"))
}

#[expect(
    clippy::needless_pass_by_value,
    reason = "Test helper keeps call sites compact and mirrors asserted values."
)]
fn verify_extraction(json: &str, expected: Option<PathBuf>) {
    let msg = parse_message(json);
    assert_eq!(extract_test_executable(&msg), expected);
}

#[test]
fn extract_test_executable_filters_non_tests() {
    verify_extraction(
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
        None,
    );
}

#[test]
fn extract_test_executable_accepts_tests() {
    verify_extraction(
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
        Some(PathBuf::from("/tmp/test-bin")),
    );
}
