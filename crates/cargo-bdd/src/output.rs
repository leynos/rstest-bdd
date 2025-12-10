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

#[derive(Clone, Copy)]
#[allow(clippy::struct_excessive_bools)]
pub(crate) struct ScenarioDisplayOptions {
    pub include_line: bool,
    pub include_tags: bool,
    pub include_reason: bool,
    pub insert_leading_newline: bool,
}

impl Default for ScenarioDisplayOptions {
    fn default() -> Self {
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
    let location = format_scenario_location(scenario, options.include_line);
    let mut line = format!("skipped {location} :: {}", scenario.name);
    append_scenario_annotations(&mut line, scenario);
    append_scenario_tags(&mut line, scenario, options.include_tags);
    append_scenario_reason(&mut line, scenario, options.include_reason);
    writeln!(writer, "{line}").wrap_err_with(|| {
        format!(
            "failed to write scenario status for {} :: {}",
            scenario.feature_path, scenario.name
        )
    })
}

fn format_scenario_location(scenario: &Scenario, include_line: bool) -> String {
    if include_line && scenario.line != 0 {
        format!("{}:{}", scenario.feature_path, scenario.line)
    } else {
        scenario.feature_path.clone()
    }
}

fn append_scenario_annotations(line: &mut String, scenario: &Scenario) {
    if scenario.forced_failure {
        line.push_str(" [forced failure]");
    }
    if !scenario.allow_skipped && !scenario.forced_failure {
        line.push_str(" [skip disallowed]");
    }
}

fn append_scenario_tags(line: &mut String, scenario: &Scenario, include_tags: bool) {
    if include_tags && !scenario.tags.is_empty() {
        line.push_str(" [tags: ");
        line.push_str(&scenario.tags.join(", "));
        line.push(']');
    }
}

fn append_scenario_reason(line: &mut String, scenario: &Scenario, include_reason: bool) {
    if include_reason {
        if let Some(message) = &scenario.message {
            line.push_str(" - ");
            line.push_str(message);
        }
    }
}

pub(crate) fn write_bypassed_steps(writer: &mut dyn Write, steps: &[BypassedStep]) -> Result<()> {
    for step in steps {
        let location = if step.scenario_line == 0 {
            step.feature_path.clone()
        } else {
            format!("{}:{}", step.feature_path, step.scenario_line)
        };
        let mut line = format!(
            "{} '{}' ({}:{}) - skipped in {} :: {}",
            step.keyword, step.pattern, step.file, step.line, location, step.scenario_name,
        );
        if !step.tags.is_empty() {
            line.push_str(" [tags: ");
            line.push_str(&step.tags.join(", "));
            line.push(']');
        }
        if let Some(reason) = &step.reason {
            line.push_str(" - ");
            line.push_str(reason);
        }
        writeln!(writer, "{line}").wrap_err_with(|| {
            format!(
                "failed to write bypassed step {} '{}' at {}:{}",
                step.keyword, step.pattern, step.file, step.line
            )
        })?;
    }
    Ok(())
}
