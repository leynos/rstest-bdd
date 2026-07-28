//! Snapshot and property tests for scenario output formatting.

use proptest::prelude::*;
use rstest::rstest;

use super::*;
use crate::registry::ScenarioOutcome;

fn sample_scenario() -> Scenario {
    Scenario {
        feature_path: "features/checkout.feature".to_owned(),
        name: "declined card is rejected".to_owned(),
        status: ScenarioOutcome::Skipped,
        message: Some("payment gateway sandbox unavailable".to_owned()),
        allow_skipped: true,
        forced_failure: false,
        line: 42,
        tags: vec!["payments".to_owned(), "slow".to_owned()],
    }
}

/// Decode rendered bytes, failing loudly on the (unreachable) invalid-UTF-8
/// case rather than snapshotting a silent `<invalid utf-8>` placeholder.
/// `let ... else { panic! }` keeps the loud failure without tripping the
/// workspace `expect`/`unwrap` restrictions in this non-`#[test]` helper.
fn render_to_string(buffer: Vec<u8>) -> String {
    let Ok(output) = String::from_utf8(buffer) else {
        panic!("rendered output must be valid UTF-8");
    };
    output
}

fn render_scenarios(scenarios: &[Scenario], options: ScenarioDisplayOptions) -> String {
    let mut buffer = Vec::new();
    let result = write_scenarios(&mut buffer, scenarios, options);
    assert!(result.is_ok(), "rendering into Vec<u8> should not fail");
    render_to_string(buffer)
}

/// A skipped scenario whose annotations render. `sample_scenario` keeps
/// `allow_skipped: true, forced_failure: false`, so its snapshots never
/// exercise `append_scenario_annotations`; this one forces the
/// `[forced failure]` marker so a regression in that fragment (or its
/// ordering relative to tags and the reason) is caught.
fn annotated_scenario() -> Scenario {
    Scenario {
        forced_failure: true,
        allow_skipped: false,
        ..sample_scenario()
    }
}

#[rstest]
#[case::with_reasons(
    sample_scenario(),
    ScenarioDisplayOptions::with_reasons(),
    "scenarios_with_reasons"
)]
#[case::compact(
    sample_scenario(),
    ScenarioDisplayOptions::compact(),
    "scenarios_compact"
)]
#[case::step_listing_appendix(
    sample_scenario(),
    ScenarioDisplayOptions::step_listing_appendix(),
    "scenarios_step_listing_appendix"
)]
#[case::forced_failure(
    annotated_scenario(),
    ScenarioDisplayOptions::with_reasons(),
    "scenarios_forced_failure"
)]
fn snapshot_scenario_modes(
    #[case] scenario: Scenario,
    #[case] options: ScenarioDisplayOptions,
    #[case] snapshot: &str,
) {
    let output = render_scenarios(&[scenario], options);
    insta::assert_snapshot!(snapshot, output);
}

#[test]
fn snapshot_bypassed_steps() {
    let steps = [BypassedStep {
        keyword: "Given".to_owned(),
        pattern: "a declined card".to_owned(),
        file: "tests/steps.rs".to_owned(),
        line: 7,
        feature_path: "features/checkout.feature".to_owned(),
        scenario_name: "declined card is rejected".to_owned(),
        scenario_line: 42,
        tags: vec!["payments".to_owned()],
        reason: Some("sandbox unavailable".to_owned()),
    }];
    let mut buffer = Vec::new();
    let result = write_bypassed_steps(&mut buffer, &steps);
    assert!(result.is_ok(), "rendering into Vec<u8> should not fail");
    let output = render_to_string(buffer);
    insta::assert_snapshot!("bypassed_steps", output);
}

/// Strategy producing a scenario with arbitrary metadata.
fn scenario_strategy() -> impl Strategy<Value = Scenario> {
    (
        "[a-z/]{1,20}\\.feature",
        "[a-zA-Z ]{1,24}",
        proptest::option::of("[a-zA-Z ]{1,24}"),
        any::<bool>(),
        any::<bool>(),
        0u32..200,
        proptest::collection::vec("[a-z]{1,8}", 0..4),
    )
        .prop_map(
            |(feature_path, name, message, allow_skipped, forced_failure, line, tags)| Scenario {
                feature_path,
                name,
                status: ScenarioOutcome::Skipped,
                message,
                allow_skipped,
                forced_failure,
                line,
                tags,
            },
        )
}

proptest! {
    /// Structural invariants of the rendered scenario line.
    #[test]
    fn rendered_line_structure_matches_options(
        scenario in scenario_strategy(),
        include_line in any::<bool>(),
        include_tags in any::<bool>(),
        include_reason in any::<bool>(),
    ) {
        let options = ScenarioDisplayOptions {
            include_line,
            include_tags,
            include_reason,
            insert_leading_newline: false,
        };
        let line = format_scenario_line(&scenario, options);

        let expect_tags = include_tags && !scenario.tags.is_empty();
        prop_assert_eq!(line.contains(" [tags: "), expect_tags);

        let expect_line_suffix = include_line && scenario.line != 0;
        let location_with_line =
            format!("{}:{}", scenario.feature_path, scenario.line);
        prop_assert_eq!(
            line.contains(&location_with_line),
            expect_line_suffix
        );

        // The ` - {message}` reason suffix appears iff reasons are included AND
        // a message exists; `append_reason` renders nothing otherwise. The
        // generated alphabets contain no `-`, so ` - ` cannot appear elsewhere
        // in the line — letting us assert both the message-present and
        // message-absent halves of the invariant (the `None` half is the
        // `append_reason` early return).
        let reason_rendered = include_reason && scenario.message.is_some();
        prop_assert_eq!(line.contains(" - "), reason_rendered);
        if let Some(message) = &scenario.message {
            let fragment = format!(" - {message}");
            prop_assert_eq!(line.contains(&fragment), reason_rendered);
        }
    }

    /// The leading blank separator appears iff requested (and the listing
    /// is non-empty).
    #[test]
    fn leading_newline_appears_iff_requested(
        scenario in scenario_strategy(),
        insert_leading_newline in any::<bool>(),
    ) {
        let options = ScenarioDisplayOptions {
            include_line: false,
            include_tags: false,
            include_reason: false,
            insert_leading_newline,
        };
        let output = render_scenarios(std::slice::from_ref(&scenario), options);
        prop_assert_eq!(output.starts_with('\n'), insert_leading_newline);
    }
}
