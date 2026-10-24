//! Unit tests for command-line argument parsing.

use super::*;

#[test]
fn write_skip_reports_json_emits_fields() {
    let report = SkipReport {
        feature: "feature",
        scenario: "scenario",
        line: 3,
        tags: &[String::from("@a")],
        libraries: &[String::from("accounts"), String::from("filesystem")],
        reason: Some("why"),
        step: Some(SkippedDefinition {
            library: "accounts",
            keyword: "Given",
            pattern: "x",
            file: "file",
            line: 7,
        }),
    };
    let mut buffer = Vec::new();
    serde_json::to_writer(&mut buffer, &[report]).expect("test setup should succeed");
    let parsed: serde_json::Value =
        serde_json::from_slice(&buffer).expect("test setup should succeed");
    let entry = parsed
        .as_array()
        .and_then(|array| array.first())
        .ok_or_else(|| eyre::eyre!("missing entry"))
        .expect("test setup should succeed");
    assert_eq!(
        entry.get("feature"),
        Some(&serde_json::Value::String("feature".into()))
    );
    assert_eq!(
        entry.get("scenario"),
        Some(&serde_json::Value::String("scenario".into()))
    );
    assert_eq!(entry.get("line"), Some(&serde_json::Value::from(3_u64)));
    assert_eq!(
        entry.get("reason"),
        Some(&serde_json::Value::String("why".into()))
    );
    assert_eq!(
        entry.get("libraries"),
        Some(&serde_json::json!(["accounts", "filesystem"]))
    );
    let step = entry
        .get("step")
        .and_then(serde_json::Value::as_object)
        .ok_or_else(|| eyre::eyre!("missing step object"))
        .expect("test setup should succeed");
    assert_eq!(
        step.get("keyword"),
        Some(&serde_json::Value::String("Given".into()))
    );
    assert_eq!(
        step.get("pattern"),
        Some(&serde_json::Value::String("x".into()))
    );
    assert_eq!(
        step.get("library"),
        Some(&serde_json::Value::String("accounts".into()))
    );
}

#[test]
fn duplicate_groups_do_not_cross_library_boundaries() {
    let steps = [
        Step {
            library: "accounts".to_owned(),
            keyword: "Given".to_owned(),
            pattern: "the domain is empty".to_owned(),
            file: "accounts.rs".to_owned(),
            line: 1,
            used: false,
        },
        Step {
            library: "filesystem".to_owned(),
            keyword: "Given".to_owned(),
            pattern: "the domain is empty".to_owned(),
            file: "filesystem.rs".to_owned(),
            line: 1,
            used: false,
        },
    ];

    assert!(group_duplicate_steps(steps).is_empty());
}

#[test]
fn duplicate_groups_are_ordered_by_library_keyword_and_pattern() {
    let step = |library: &str, keyword: &str, pattern: &str| Step {
        library: library.to_owned(),
        keyword: keyword.to_owned(),
        pattern: pattern.to_owned(),
        file: "steps.rs".to_owned(),
        line: 1,
        used: false,
    };
    let steps = [
        step("filesystem", "Then", "z"),
        step("accounts", "Given", "a"),
        step("filesystem", "Then", "z"),
        step("accounts", "Given", "a"),
    ];

    let keys = group_duplicate_steps(steps)
        .into_iter()
        .map(|group| {
            let first = group.first().expect("duplicate group must be non-empty");
            (
                first.library.clone(),
                first.keyword.clone(),
                first.pattern.clone(),
            )
        })
        .collect::<Vec<_>>();

    assert_eq!(
        keys,
        [
            ("accounts".into(), "Given".into(), "a".into()),
            ("filesystem".into(), "Then".into(), "z".into()),
        ]
    );
}
