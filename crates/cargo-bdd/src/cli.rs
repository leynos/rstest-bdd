//! Command dispatch and formatting for the `cargo bdd` entrypoint.

use std::collections::HashMap;
use std::io::{self, Write};

use clap::{Args, Parser, Subcommand};
use eyre::{bail, Context, Result};
use serde::Serialize;

use crate::output::{
    write_bypassed_steps, write_group_separator, write_scenarios, write_step,
    ScenarioDisplayOptions,
};
use crate::registry::{collect_registry, BypassedStep, Scenario, ScenarioOutcome, Step};

/// Cargo subcommand providing diagnostics for rstest-bdd.
#[derive(Parser)]
#[command(author, version, about)]
pub(crate) struct Cli {
    #[command(subcommand)]
    command: Commands,
}

/// Supported diagnostic commands.
#[derive(Subcommand)]
pub(crate) enum Commands {
    /// List all registered steps.
    Steps(StepsArgs),
    /// List registered steps that were never executed.
    Unused,
    /// List step definitions that share the same keyword and pattern.
    Duplicates,
    /// List skipped scenarios and their reasons.
    Skipped(SkippedArgs),
}

#[derive(Args)]
pub(crate) struct StepsArgs {
    /// Filter for step definitions bypassed when scenarios were skipped.
    #[arg(long)]
    pub skipped: bool,
    /// Emit JSON instead of human-readable text.
    #[arg(long, requires = "skipped")]
    pub json: bool,
}

#[derive(Args)]
pub(crate) struct SkippedArgs {
    /// Include file/line information and skip reasons.
    #[arg(long)]
    pub reasons: bool,
    /// Emit JSON instead of human-readable text.
    #[arg(long)]
    pub json: bool,
}

#[derive(Serialize)]
struct SkipReport<'a> {
    feature: &'a str,
    scenario: &'a str,
    line: u32,
    tags: &'a [String],
    #[serde(skip_serializing_if = "Option::is_none")]
    reason: Option<&'a str>,
    #[serde(skip_serializing_if = "Option::is_none")]
    step: Option<SkippedDefinition<'a>>,
}

#[derive(Serialize)]
struct SkippedDefinition<'a> {
    keyword: &'a str,
    pattern: &'a str,
    file: &'a str,
    line: u32,
}

impl<'a> From<&'a Scenario> for SkipReport<'a> {
    fn from(scenario: &'a Scenario) -> Self {
        Self {
            feature: &scenario.feature_path,
            scenario: &scenario.name,
            line: scenario.line,
            tags: &scenario.tags,
            reason: scenario.message.as_deref(),
            step: None,
        }
    }
}

impl<'a> From<&'a BypassedStep> for SkipReport<'a> {
    fn from(step: &'a BypassedStep) -> Self {
        Self {
            feature: &step.feature_path,
            scenario: &step.scenario_name,
            line: step.scenario_line,
            tags: &step.tags,
            reason: step.reason.as_deref(),
            step: Some(SkippedDefinition {
                keyword: &step.keyword,
                pattern: &step.pattern,
                file: &step.file,
                line: step.line,
            }),
        }
    }
}

pub(crate) fn run() -> Result<()> {
    match Cli::parse().command {
        Commands::Steps(args) => handle_steps(&args)?,
        Commands::Unused => handle_unused()?,
        Commands::Duplicates => handle_duplicates()?,
        Commands::Skipped(args) => handle_skipped(&args)?,
    }
    Ok(())
}

fn handle_steps(args: &StepsArgs) -> Result<()> {
    if args.skipped {
        return handle_bypassed_steps(args.json);
    }
    if args.json {
        bail!("--json is only supported together with --skipped");
    }
    write_filtered_steps(|_| true, Some(ScenarioDisplayOptions::default()))
}

fn handle_unused() -> Result<()> {
    write_filtered_steps(|step| !step.used, None)
}

fn handle_duplicates() -> Result<()> {
    let mut groups: HashMap<(String, String), Vec<Step>> = HashMap::new();
    for step in collect_registry()?.steps {
        groups
            .entry((step.keyword.clone(), step.pattern.clone()))
            .or_default()
            .push(step);
    }
    let mut stdout = io::stdout();
    for group in groups.into_values().filter(|g| g.len() > 1) {
        for step in &group {
            write_step(&mut stdout, step)?;
        }
        write_group_separator(&mut stdout)?;
    }
    stdout
        .flush()
        .wrap_err("failed to flush duplicate listing to stdout")
}

fn handle_bypassed_steps(json: bool) -> Result<()> {
    let registry = collect_registry()?;
    if json {
        let reports: Vec<_> = registry
            .bypassed_steps
            .iter()
            .map(SkipReport::from)
            .collect();
        return write_skip_reports_json(&reports);
    }

    let mut stdout = io::stdout();
    write_bypassed_steps(&mut stdout, &registry.bypassed_steps)?;
    stdout
        .flush()
        .wrap_err("failed to flush bypassed step listing")
}

fn handle_skipped(args: &SkippedArgs) -> Result<()> {
    let registry = collect_registry()?;
    let skipped: Vec<_> = registry
        .scenarios
        .iter()
        .filter(|scenario| scenario.status == ScenarioOutcome::Skipped)
        .collect();

    if args.json {
        let reports: Vec<_> = skipped
            .iter()
            .map(|scenario| SkipReport::from(*scenario))
            .collect();
        return write_skip_reports_json(&reports);
    }

    let options = if args.reasons {
        ScenarioDisplayOptions {
            include_line: true,
            include_tags: true,
            include_reason: true,
            insert_leading_newline: false,
        }
    } else {
        ScenarioDisplayOptions {
            include_line: false,
            include_tags: false,
            include_reason: false,
            insert_leading_newline: false,
        }
    };

    let mut stdout = io::stdout();
    write_scenarios(&mut stdout, &registry.scenarios, options)?;
    stdout
        .flush()
        .wrap_err("failed to flush skipped scenario listing")
}

fn write_filtered_steps<F>(filter: F, scenarios: Option<ScenarioDisplayOptions>) -> Result<()>
where
    F: Fn(&Step) -> bool,
{
    let registry = collect_registry()?;
    let mut stdout = io::stdout();
    registry
        .steps
        .iter()
        .filter(|&step| filter(step))
        .try_for_each(|step| write_step(&mut stdout, step))?;
    if let Some(options) = scenarios {
        write_scenarios(&mut stdout, &registry.scenarios, options)?;
    }
    stdout
        .flush()
        .wrap_err("failed to flush step listing to stdout")
}

fn write_skip_reports_json(reports: &[SkipReport<'_>]) -> Result<()> {
    let mut stdout = io::stdout();
    serde_json::to_writer(&mut stdout, reports)
        .wrap_err("failed to serialize skip diagnostics to JSON")?;
    stdout
        .write_all(b"\n")
        .wrap_err("failed to terminate JSON output with newline")?;
    stdout.flush().wrap_err("failed to flush JSON output")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn write_skip_reports_json_emits_fields() -> eyre::Result<()> {
        let report = SkipReport {
            feature: "feature",
            scenario: "scenario",
            line: 3,
            tags: &[String::from("@a")],
            reason: Some("why"),
            step: Some(SkippedDefinition {
                keyword: "Given",
                pattern: "x",
                file: "file",
                line: 7,
            }),
        };
        let mut buffer = Vec::new();
        serde_json::to_writer(&mut buffer, &[report])?;
        let parsed: serde_json::Value = serde_json::from_slice(&buffer)?;
        let entry = parsed
            .as_array()
            .and_then(|array| array.first())
            .ok_or_else(|| eyre::eyre!("missing entry"))?;
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
        let step = entry
            .get("step")
            .and_then(serde_json::Value::as_object)
            .ok_or_else(|| eyre::eyre!("missing step object"))?;
        assert_eq!(
            step.get("keyword"),
            Some(&serde_json::Value::String("Given".into()))
        );
        assert_eq!(
            step.get("pattern"),
            Some(&serde_json::Value::String("x".into()))
        );
        Ok(())
    }
}
