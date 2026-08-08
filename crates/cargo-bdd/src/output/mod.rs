//! Helpers for rendering diagnostic output.

use std::io::Write;

use eyre::{Context, Result};

use crate::registry::{BypassedStep, Scenario, ScenarioOutcome, Step};

/// Write one step-listing line: `Keyword 'pattern' (file:line)`.
///
/// Emits a trailing newline. Errors carry the step's identity so a failed
/// write names the offending entry.
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

/// Write the `---` separator that divides duplicate-step groups.
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
    pub(crate) const fn compact() -> Self {
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
    pub(crate) const fn with_reasons() -> Self {
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
    pub(crate) const fn step_listing_appendix() -> Self {
        Self {
            include_line: false,
            include_tags: false,
            include_reason: true,
            insert_leading_newline: true,
        }
    }
}

/// Write the skipped-scenario listing, one line per scenario.
///
/// Scenarios that are not `Skipped` are filtered out. When none remain,
/// nothing at all is written — not even the separator — so an empty listing
/// leaves the preceding output untouched. Otherwise, when
/// `options.insert_leading_newline` is set, a single blank line is emitted
/// first to separate the listing from a preceding step listing.
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

/// Write one rendered scenario line, with a trailing newline.
///
/// The line itself comes from [`format_scenario_line`]; this function owns
/// only the write and its error context.
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

/// Write one line per bypassed step, each with a trailing newline.
///
/// Each line names the step and its source location, then the scenario that
/// skipped it, before the shared [`append_tags`] and [`append_reason`]
/// fragments. Both fragments are always included here, unlike the scenario
/// listing, which gates them on [`ScenarioDisplayOptions`].
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
mod tests;
