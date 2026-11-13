//! Unit tests for command-line argument parsing.

use std::ffi::OsString;

use super::*;

/// Both invocation forms select the same `steps` subcommand.
#[test]
fn flat_and_cargo_dispatched_forms_select_steps() {
    for arguments in [
        vec!["cargo-bdd", "steps"],
        vec!["cargo-bdd", "bdd", "steps"],
    ] {
        let arguments = arguments.into_iter().map(OsString::from);
        let (cli, matches, _) =
            parse_command_line(arguments).expect("supported invocation should parse");

        assert!(matches!(cli.command, Commands::Steps(_)));
        assert!(selected_matches(&matches, "steps").is_ok());
    }
}

/// The missing `--skipped` failure is a Clap error, not a process exit.
#[test]
fn missing_skipped_is_returned_as_a_clap_failure() {
    let error = missing_required_skipped_error(InvocationForm::Direct);
    let CliError::Clap(error) = error else {
        panic!("the missing `--skipped` failure should be a Clap error");
    };

    assert_eq!(
        error.kind(),
        clap::error::ErrorKind::MissingRequiredArgument
    );
    assert_eq!(error.exit_code(), 2);
    assert!(
        error.render().to_string().contains("--skipped"),
        "the rendered failure should name the missing argument"
    );
}

/// An unknown subcommand is returned so the caller can render it.
#[test]
fn unknown_subcommands_are_returned_instead_of_exiting() {
    let result = parse_command_line(["cargo-bdd", "unknown-command"].map(OsString::from));
    let Err(CliError::Clap(error)) = result else {
        panic!("an unknown subcommand should be a Clap error");
    };

    assert_eq!(error.kind(), clap::error::ErrorKind::InvalidSubcommand);
}

/// Serialized skip reports carry every documented JSON field.
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

/// Identical patterns in different libraries are not duplicates.
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

/// Duplicate groups are returned in deterministic key order.
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

/// The unused listing keeps only steps no scenario executed.
#[test]
fn unused_listing_keeps_only_unexecuted_steps() {
    let step = |pattern: &str, used: bool| Step {
        library: "accounts".to_owned(),
        keyword: "Given".to_owned(),
        pattern: pattern.to_owned(),
        file: "steps.rs".to_owned(),
        line: 1,
        used,
    };
    let steps = [step("executed", true), step("dormant", false)];
    let mut rendered = Vec::new();

    write_unused_steps(&mut rendered, &steps).expect("writing to a buffer cannot fail");

    let rendered = String::from_utf8(rendered).expect("the listing is UTF-8");
    assert!(
        rendered.contains("dormant"),
        "the unexecuted step should be listed"
    );
    assert!(
        !rendered.contains("executed '"),
        "an executed step must not be listed"
    );
}

/// The duplicate listing separates each group with the documented marker.
#[test]
fn duplicate_listing_writes_a_separator_after_each_group() {
    let step = |pattern: &str, line: u32| Step {
        library: "accounts".to_owned(),
        keyword: "Given".to_owned(),
        pattern: pattern.to_owned(),
        file: "steps.rs".to_owned(),
        line,
        used: false,
    };
    let steps = [step("repeated", 1), step("repeated", 2), step("unique", 3)];
    let mut rendered = Vec::new();

    write_duplicate_groups(&mut rendered, &steps).expect("writing to a buffer cannot fail");

    let rendered = String::from_utf8(rendered).expect("the listing is UTF-8");
    assert_eq!(
        rendered.matches("---\n").count(),
        1,
        "only the duplicated group should be separated"
    );
    assert!(
        !rendered.contains("unique"),
        "a step with no duplicate must not be listed"
    );
}

/// The bypassed-step listing switches between JSON and text rendering.
#[test]
fn bypassed_listing_honours_the_json_switch() {
    let steps = [BypassedStep {
        library: "accounts".to_owned(),
        keyword: "Given".to_owned(),
        pattern: "x".to_owned(),
        file: "steps.rs".to_owned(),
        line: 4,
        feature_path: "features/a.feature".to_owned(),
        scenario_name: "scenario".to_owned(),
        scenario_line: 2,
        tags: Vec::new(),
        libraries: Vec::new(),
        reason: Some("why".to_owned()),
    }];

    let mut json = Vec::new();
    write_bypassed_steps_listing(&mut json, &steps, true).expect("JSON rendering should succeed");
    let parsed: serde_json::Value =
        serde_json::from_slice(&json).expect("the JSON listing should parse");
    assert_eq!(
        parsed
            .as_array()
            .and_then(|array| array.first())
            .and_then(|entry| entry.get("scenario")),
        Some(&serde_json::Value::String("scenario".into()))
    );

    let mut text = Vec::new();
    write_bypassed_steps_listing(&mut text, &steps, false).expect("text rendering should succeed");
    let text = String::from_utf8(text).expect("the listing is UTF-8");
    assert!(
        text.starts_with("Given 'x'"),
        "text rendering starts with the step"
    );
    assert!(
        !text.starts_with('['),
        "text rendering must not be a JSON array"
    );
}

/// The skipped listing reports only skipped scenarios, in JSON or text.
#[test]
fn skipped_listing_reports_only_skipped_scenarios() {
    let scenario = |name: &str, status: ScenarioOutcome| Scenario {
        feature_path: "features/a.feature".to_owned(),
        name: name.to_owned(),
        status,
        message: None,
        allow_skipped: true,
        forced_failure: false,
        line: 1,
        tags: Vec::new(),
        libraries: Vec::new(),
    };
    let scenarios = [
        scenario("skipped one", ScenarioOutcome::Skipped),
        scenario("passed one", ScenarioOutcome::Passed),
    ];

    let mut json = Vec::new();
    write_skipped_listing(
        &mut json,
        &scenarios,
        &SkippedSettings {
            reasons: false,
            json: true,
        },
    )
    .expect("JSON rendering should succeed");
    let rendered = String::from_utf8(json).expect("the listing is UTF-8");
    assert!(
        rendered.contains("skipped one"),
        "the skipped scenario should be reported"
    );
    assert!(
        !rendered.contains("passed one"),
        "a passing scenario must not be reported"
    );

    let mut verbose = Vec::new();
    write_skipped_listing(
        &mut verbose,
        &scenarios,
        &SkippedSettings {
            reasons: true,
            json: false,
        },
    )
    .expect("text rendering should succeed");
    let verbose = String::from_utf8(verbose).expect("the listing is UTF-8");
    assert!(
        verbose.contains("skipped one") && !verbose.contains("passed one"),
        "text rendering must apply the same filter"
    );
}

/// The Cargo-dispatched form reports the frozen `--skipped` requirement.
///
/// Clap measured this message before `OrthoConfig` was adopted; the checked-in
/// baseline fixture pins the direct form, and this covers the wrapper form
/// that shares the wording.
#[test]
fn cargo_dispatched_missing_skipped_matches_the_frozen_message() {
    let error = missing_required_skipped_error(InvocationForm::CargoDispatched);
    let CliError::Clap(error) = error else {
        panic!("the missing `--skipped` failure should be a Clap error");
    };

    assert_eq!(
        error.kind(),
        clap::error::ErrorKind::MissingRequiredArgument
    );
    assert_eq!(error.exit_code(), 2);
    let rendered = error.render().to_string();
    assert!(
        rendered.contains("the following required arguments were not provided:\n  --skipped"),
        "the wrapper form should keep Clap's wording, got: {rendered}"
    );
}
