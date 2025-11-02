//! Helpers for rendering diagnostic output.

use std::io::Write;

use eyre::{Context, Result};

use crate::{Scenario, ScenarioOutcome, Step};

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

pub(crate) fn write_scenarios(writer: &mut dyn Write, scenarios: &[Scenario]) -> Result<()> {
    let skipped: Vec<_> = scenarios
        .iter()
        .filter(|scenario| scenario.status == ScenarioOutcome::Skipped)
        .collect();
    if skipped.is_empty() {
        return Ok(());
    }
    writeln!(writer).wrap_err("failed to separate step and scenario listings")?;
    for scenario in skipped {
        write_scenario(writer, scenario)?;
    }
    Ok(())
}

fn write_scenario(writer: &mut dyn Write, scenario: &Scenario) -> Result<()> {
    let mut line = format!("skipped {} :: {}", scenario.feature_path, scenario.name);
    if scenario.forced_failure {
        line.push_str(" [forced failure]");
    }
    if !scenario.allow_skipped && !scenario.forced_failure {
        line.push_str(" [skip disallowed]");
    }
    if let Some(message) = &scenario.message {
        line.push_str(" - ");
        line.push_str(message);
    }
    writeln!(writer, "{line}").wrap_err_with(|| {
        format!(
            "failed to write scenario status for {} :: {}",
            scenario.feature_path, scenario.name
        )
    })
}
