//! Helpers for rendering diagnostic output.

use std::io::Write;

use eyre::{Context, Result};

use crate::registry::{BypassedStep, Scenario, ScenarioOutcome, Step};

pub(crate) fn write_step(writer: &mut dyn Write, step: &Step) -> Result<()> {
    writeln!(
        writer,
        "{} '{}' ({}:{})",
        step.keyword, step.pattern, step.file, step.line
    )
    .wrap_err_with(|| {
        format!(
            "failed to write step {} '{}' at {}:{}",
            step.keyword, step.pattern, step.file, step.line
        )
    })
}

pub(crate) fn write_group_separator(writer: &mut dyn Write) -> Result<()> {
    writeln!(writer, "---").wrap_err("failed to write duplicate separator")
}

/// Rendering options for skipped-scenario listings.
///
/// Construct via the named constructors so call sites express intent rather
/// than positional booleans: [`Self::compact`], [`Self::with_reasons`], or
/// [`Self::step_listing_appendix`].
#[derive(Clone, Copy)]
#[expect(
    clippy::struct_excessive_bools,
    reason = "Rendering flags are independent CLI switches; booleans keep call sites readable."
)]
pub(crate) struct ScenarioDisplayOptions {
    /// Append `:line` to the feature path when the line is known.
    pub include_line: bool,
    /// Append the `[tags: …]` fragment when the scenario has tags.
    pub include_tags: bool,
    /// Append the ` - reason` fragment when a skip reason was recorded.
    pub include_reason: bool,
    /// Emit a blank separator line before the scenario listing.
    pub insert_leading_newline: bool,
}

impl ScenarioDisplayOptions {
    /// Minimal listing: feature path and scenario name only.
    ///
    /// Used by `cargo bdd skipped` without `--reasons`.
    pub(crate) fn compact() -> Self {
        Self {
            include_line: false,
            include_tags: false,
            include_reason: false,
            insert_leading_newline: false,
        }
    }

    /// Detailed listing with location line, tags, and skip reasons.
    ///
    /// Used by `cargo bdd skipped --reasons`.
    pub(crate) fn with_reasons() -> Self {
        Self {
            include_line: true,
            include_tags: true,
            include_reason: true,
            insert_leading_newline: false,
        }
    }

    /// Listing appended after a step listing: skip reasons only, separated
    /// from the preceding output by a blank line.
    ///
    /// Used by `cargo bdd steps`.
    pub(crate) fn step_listing_appendix() -> Self {
        Self {
            include_line: false,
            include_tags: false,
            include_reason: true,
            insert_leading_newline: true,
        }
    }
}

pub(crate) fn write_scenarios(
    writer: &mut dyn Write,
    scenarios: &[Scenario],
    options: ScenarioDisplayOptions,
) -> Result<()> {
    let skipped: Vec<_> = scenarios
        .iter()
        .filter(|scenario| scenario.status == ScenarioOutcome::Skipped)
        .collect();
    if skipped.is_empty() {
        return Ok(());
    }
    if options.insert_leading_newline {
        writeln!(writer).wrap_err("failed to separate step and scenario listings")?;
    }
    for scenario in skipped {
        write_scenario(writer, scenario, options)?;
    }
    Ok(())
}

fn write_scenario(
    writer: &mut dyn Write,
    scenario: &Scenario,
    options: ScenarioDisplayOptions,
) -> Result<()> {
    let line = format_scenario_line(scenario, options);
    writeln!(writer, "{line}").wrap_err_with(|| {
        format!(
            "failed to write scenario status for {} :: {}",
            scenario.feature_path, scenario.name
        )
    })
}

/// Render one skipped-scenario line according to `options`.
///
/// This is the canonical scenario formatter: the location, tag, and reason
/// fragments come from the shared [`format_location`], [`append_tags`], and
/// [`append_reason`] helpers (also used for bypassed steps), gated by the
/// display options rather than duplicated per mode.
fn format_scenario_line(scenario: &Scenario, options: ScenarioDisplayOptions) -> String {
    let rendered_line = if options.include_line {
        scenario.line
    } else {
        0
    };
    let location = format_location(&scenario.feature_path, rendered_line);
    let mut line = format!("skipped {location} :: {}", scenario.name);
    append_scenario_annotations(&mut line, scenario);
    if options.include_tags {
        append_tags(&mut line, &scenario.tags);
    }
    if options.include_reason {
        append_reason(&mut line, scenario.message.as_deref());
    }
    line
}

/// Render `path`, appending `:line` when `line` is non-zero (zero means the
/// line is unknown or suppressed).
fn format_location(path: &str, line: u32) -> String {
    if line == 0 {
        path.to_owned()
    } else {
        format!("{path}:{line}")
    }
}

/// Append a ` [tags: …]` fragment to `line` in place; empty tag lists append
/// nothing.
fn append_tags(line: &mut String, tags: &[String]) {
    if tags.is_empty() {
        return;
    }
    line.push_str(" [tags: ");
    line.push_str(&tags.join(", "));
    line.push(']');
}

/// Append a ` - reason` fragment to `line` in place; `None` appends nothing.
fn append_reason(line: &mut String, reason: Option<&str>) {
    let Some(message) = reason else {
        return;
    };
    line.push_str(" - ");
    line.push_str(message);
}

/// Append the scenario policy annotations (`[forced failure]` /
/// `[skip disallowed]`) to `line` in place.
fn append_scenario_annotations(line: &mut String, scenario: &Scenario) {
    if scenario.forced_failure {
        line.push_str(" [forced failure]");
    }
    if !scenario.allow_skipped && !scenario.forced_failure {
        line.push_str(" [skip disallowed]");
    }
}

pub(crate) fn write_bypassed_steps(writer: &mut dyn Write, steps: &[BypassedStep]) -> Result<()> {
    for step in steps {
        let location = format_location(&step.feature_path, step.scenario_line);
        let mut line = format!(
            "{} '{}' ({}:{}) - skipped in {} :: {}",
            step.keyword, step.pattern, step.file, step.line, location, step.scenario_name,
        );
        append_tags(&mut line, &step.tags);
        append_reason(&mut line, step.reason.as_deref());
        writeln!(writer, "{line}").wrap_err_with(|| {
            format!(
                "failed to write bypassed step {} '{}' at {}:{}",
                step.keyword, step.pattern, step.file, step.line
            )
        })?;
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    //! Snapshot and property tests for scenario output formatting.

    use proptest::prelude::*;

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

    fn render_scenarios(scenarios: &[Scenario], options: ScenarioDisplayOptions) -> String {
        let mut buffer = Vec::new();
        let result = write_scenarios(&mut buffer, scenarios, options);
        assert!(result.is_ok(), "rendering into Vec<u8> should not fail");
        String::from_utf8(buffer).unwrap_or_else(|_| String::from("<invalid utf-8>"))
    }

    #[test]
    fn snapshot_with_reasons_mode() {
        let output = render_scenarios(&[sample_scenario()], ScenarioDisplayOptions::with_reasons());
        insta::assert_snapshot!("scenarios_with_reasons", output);
    }

    #[test]
    fn snapshot_compact_mode() {
        let output = render_scenarios(&[sample_scenario()], ScenarioDisplayOptions::compact());
        insta::assert_snapshot!("scenarios_compact", output);
    }

    #[test]
    fn snapshot_step_listing_appendix_mode() {
        let output = render_scenarios(
            &[sample_scenario()],
            ScenarioDisplayOptions::step_listing_appendix(),
        );
        insta::assert_snapshot!("scenarios_step_listing_appendix", output);
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
        #[expect(clippy::expect_used, reason = "Vec<u8> writes cannot fail")]
        write_bypassed_steps(&mut buffer, &steps).expect("render bypassed steps");
        let output = String::from_utf8(buffer).unwrap_or_else(|_| String::from("<invalid utf-8>"));
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
                |(feature_path, name, message, allow_skipped, forced_failure, line, tags)| {
                    Scenario {
                        feature_path,
                        name,
                        status: ScenarioOutcome::Skipped,
                        message,
                        allow_skipped,
                        forced_failure,
                        line,
                        tags,
                    }
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

            if include_reason {
                if let Some(message) = &scenario.message {
                    let fragment = format!(" - {message}");
                    prop_assert!(line.contains(&fragment));
                }
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
}
