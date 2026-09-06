//! Command dispatch and formatting for the `cargo bdd` entrypoint.

use std::{
    collections::BTreeMap,
    io::{self, Write},
};

use clap::{Args, Parser, Subcommand};
use eyre::{Context, Result, bail};
use serde::Serialize;

use crate::{
    output::{
        ScenarioDisplayOptions,
        write_bypassed_steps,
        write_group_separator,
        write_scenarios,
        write_step,
    },
    registry::{BypassedStep, Scenario, ScenarioOutcome, Step, collect_registry},
};

/// Cargo subcommand providing diagnostics for rstest-bdd.
#[derive(Parser)]
#[command(author, version, about)]
pub(crate) struct Cli {
    #[command(subcommand)]
    /// Diagnostic command selected by the user.
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
/// Options for listing registered steps.
pub(crate) struct StepsArgs {
    /// Filter for step definitions bypassed when scenarios were skipped.
    #[arg(long)]
    pub skipped: bool,
    /// Emit JSON instead of human-readable text.
    #[arg(long, requires = "skipped")]
    pub json: bool,
}

#[derive(Args)]
/// Options for listing skipped scenarios.
pub(crate) struct SkippedArgs {
    /// Include file/line information and skip reasons.
    #[arg(long)]
    pub reasons: bool,
    /// Emit JSON instead of human-readable text.
    #[arg(long)]
    pub json: bool,
}

#[derive(Serialize)]
/// JSON representation of a skipped scenario or bypassed step.
struct SkipReport<'a> {
    /// Feature containing the skipped item.
    feature: &'a str,
    /// Scenario containing the skipped item.
    scenario: &'a str,
    /// Source line of the scenario.
    line: u32,
    /// Tags attached to the scenario.
    tags: &'a [String],
    /// Closed library identities selected by the scenario.
    libraries: &'a [String],
    #[serde(skip_serializing_if = "Option::is_none")]
    /// Optional reason recorded for the skip.
    reason: Option<&'a str>,
    #[serde(skip_serializing_if = "Option::is_none")]
    /// Optional bypassed step definition.
    step: Option<SkippedDefinition<'a>>,
}

#[derive(Serialize)]
/// JSON representation of a bypassed step definition.
struct SkippedDefinition<'a> {
    /// Stable library identity that owns the step.
    library: &'a str,
    /// Gherkin keyword for the step.
    keyword: &'a str,
    /// Registered step pattern.
    pattern: &'a str,
    /// Source file containing the step definition.
    file: &'a str,
    /// Source line containing the step definition.
    line: u32,
}

impl<'a> From<&'a Scenario> for SkipReport<'a> {
    fn from(scenario: &'a Scenario) -> Self {
        Self {
            feature: &scenario.feature_path,
            scenario: &scenario.name,
            line: scenario.line,
            tags: &scenario.tags,
            libraries: &scenario.libraries,
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
            libraries: &step.libraries,
            reason: step.reason.as_deref(),
            step: Some(SkippedDefinition {
                library: &step.library,
                keyword: &step.keyword,
                pattern: &step.pattern,
                file: &step.file,
                line: step.line,
            }),
        }
    }
}

/// Parse the command line and dispatch the selected diagnostic.
pub(crate) fn run() -> Result<()> {
    match Cli::parse().command {
        Commands::Steps(args) => handle_steps(&args)?,
        Commands::Unused => handle_unused()?,
        Commands::Duplicates => handle_duplicates()?,
        Commands::Skipped(args) => handle_skipped(&args)?,
    }
    Ok(())
}

/// Handle the `steps` diagnostic, including its optional JSON mode.
fn handle_steps(args: &StepsArgs) -> Result<()> {
    if args.skipped {
        return handle_bypassed_steps(args.json);
    }
    if args.json {
        bail!("--json is only supported together with --skipped");
    }
    write_filtered_steps(
        |_| true,
        Some(ScenarioDisplayOptions::step_listing_appendix()),
    )
}

/// Handle the `steps --unused` diagnostic.
fn handle_unused() -> Result<()> { write_filtered_steps(|step| !step.used, None) }

/// Handle the `steps --duplicates` diagnostic.
fn handle_duplicates() -> Result<()> {
    let groups = group_duplicate_steps(collect_registry()?.steps);
    let mut stdout = io::stdout();
    for group in groups {
        for step in &group {
            write_step(&mut stdout, step)?;
        }
        write_group_separator(&mut stdout)?;
    }
    stdout
        .flush()
        .wrap_err("failed to flush duplicate listing to stdout")
}

/// Group steps by the library-local identity used for duplicate diagnostics.
fn group_duplicate_steps(steps: impl IntoIterator<Item = Step>) -> Vec<Vec<Step>> {
    let mut groups: BTreeMap<(String, String, String), Vec<Step>> = BTreeMap::new();
    for step in steps {
        groups
            .entry((
                step.library.clone(),
                step.keyword.clone(),
                step.pattern.clone(),
            ))
            .or_default()
            .push(step);
    }
    groups
        .into_values()
        .filter(|group| group.len() > 1)
        .collect()
}

/// Handle the `steps --skipped` diagnostic.
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

/// Handle the `skipped` diagnostic.
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
        ScenarioDisplayOptions::with_reasons()
    } else {
        ScenarioDisplayOptions::compact()
    };

    let mut stdout = io::stdout();
    write_scenarios(&mut stdout, &registry.scenarios, options)?;
    stdout
        .flush()
        .wrap_err("failed to flush skipped scenario listing")
}

/// Write steps selected by `filter`, optionally followed by scenarios.
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

/// Serialize skip reports as newline-terminated JSON.
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
mod tests;
