//! Command dispatch and formatting for the `cargo bdd` entrypoint.

use std::{
    collections::BTreeMap,
    io::{self, Write},
};

use clap::{ArgMatches, Parser, Subcommand};
use eyre::{Context, Result, bail};
use serde::Serialize;

use crate::{
    config::{
        ConfigSources,
        SkippedConfig,
        SkippedSettings,
        StepsConfig,
        StepsSettings,
        merge_steps,
        resolve_skipped,
        validate_steps,
    },
    output::{
        ScenarioDisplayOptions,
        write_bypassed_steps,
        write_group_separator,
        write_scenarios,
        write_step,
    },
    registry::{BypassedStep, RegistryDump, Scenario, ScenarioOutcome, Step, collect_registry},
};

mod invocation;

pub(crate) use invocation::CliError;
use invocation::{InvocationForm, missing_required_skipped_error, parse_command_line};

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
    Steps(StepsConfig),
    /// List registered steps that were never executed.
    Unused,
    /// List step definitions that share the same keyword and pattern.
    Duplicates,
    /// List skipped scenarios and their reasons.
    Skipped(SkippedConfig),
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
    /// Project a skipped scenario into its report representation.
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
    /// Project a bypassed step into its report representation.
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
///
/// A Clap parse failure is returned rather than printed here, so the
/// application boundary owns the process exit while the rendered message and
/// exit status stay exactly as Clap produced them.
pub(crate) fn run() -> std::result::Result<(), CliError> {
    let (cli, matches, invocation_form) = parse_command_line(std::env::args_os())?;
    // The application edge owns the process-backed sources; the resolvers
    // below receive them, so tests can drive the same code with isolated ones.
    let sources = ConfigSources::process();
    match cli.command {
        Commands::Steps(config) => {
            handle_steps(&resolve_steps_settings(
                &config,
                &matches,
                invocation_form,
                &sources,
            )?)?;
        }
        Commands::Unused => handle_unused()?,
        Commands::Duplicates => handle_duplicates()?,
        Commands::Skipped(config) => {
            let command_matches = selected_matches(&matches, "skipped")?;
            handle_skipped(&resolve_skipped(&config, command_matches, &sources)?)?;
        }
    }
    Ok(())
}

/// Merge and validate `steps` settings before passing them to the handler.
fn resolve_steps_settings(
    config: &StepsConfig,
    matches: &ArgMatches,
    invocation_form: InvocationForm,
    sources: &ConfigSources,
) -> std::result::Result<StepsSettings, CliError> {
    let command_matches = selected_matches(matches, "steps")?;
    let merged = merge_steps(config, command_matches, sources)?;
    if command_matches.get_flag("json") && !merged.skipped {
        return Err(missing_required_skipped_error(invocation_form));
    }
    Ok(validate_steps(&merged)?)
}

/// Return the clap matches for a selected subcommand.
fn selected_matches<'a>(matches: &'a ArgMatches, name: &str) -> Result<&'a ArgMatches> {
    matches
        .subcommand_matches(name)
        .ok_or_else(|| eyre::eyre!("the parsed command did not include `{name}` matches"))
}

/// Handle the `steps` diagnostic, including its optional JSON mode.
fn handle_steps(args: &StepsSettings) -> Result<()> {
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
fn handle_unused() -> Result<()> {
    write_listing(|registry, stdout| write_unused_steps(stdout, &registry.steps))
}

/// Handle the `steps --duplicates` diagnostic.
fn handle_duplicates() -> Result<()> {
    write_listing(|registry, stdout| write_duplicate_groups(stdout, &registry.steps))
}

/// Write the unused-step listing for `steps` to `writer`.
///
/// Extracted from the handler so the selection rule is covered without
/// building a workspace registry.
fn write_unused_steps(writer: &mut dyn Write, steps: &[Step]) -> Result<()> {
    steps
        .iter()
        .filter(|step| !step.used)
        .try_for_each(|step| write_step(writer, step))
}

/// Write the duplicate-step listing for `steps` to `writer`.
///
/// Groups are separated by [`write_group_separator`], matching the CLI form
/// that operators and the checked-in baseline already expect.
fn write_duplicate_groups(writer: &mut dyn Write, steps: &[Step]) -> Result<()> {
    for group in group_duplicate_steps(steps.iter().cloned()) {
        for step in &group {
            write_step(writer, step)?;
        }
        write_group_separator(writer)?;
    }
    Ok(())
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
    write_listing(|registry, stdout| {
        write_bypassed_steps_listing(stdout, &registry.bypassed_steps, json)
    })
}

/// Write the bypassed-step listing, as JSON or as text.
fn write_bypassed_steps_listing(
    writer: &mut dyn Write,
    steps: &[BypassedStep],
    json: bool,
) -> Result<()> {
    if json {
        let reports: Vec<_> = steps.iter().map(SkipReport::from).collect();
        return write_skip_reports_json(writer, &reports);
    }
    write_bypassed_steps(writer, steps)
}

/// Handle the `skipped` diagnostic.
fn handle_skipped(args: &SkippedSettings) -> Result<()> {
    write_listing(|registry, stdout| write_skipped_listing(stdout, &registry.scenarios, args))
}

/// Write the skipped-scenario listing, as JSON or as text.
fn write_skipped_listing(
    writer: &mut dyn Write,
    scenarios: &[Scenario],
    args: &SkippedSettings,
) -> Result<()> {
    let skipped: Vec<_> = scenarios
        .iter()
        .filter(|scenario| scenario.status == ScenarioOutcome::Skipped)
        .collect();

    if args.json {
        let reports: Vec<_> = skipped
            .iter()
            .map(|scenario| SkipReport::from(*scenario))
            .collect();
        return write_skip_reports_json(writer, &reports);
    }

    let options = if args.reasons {
        ScenarioDisplayOptions::with_reasons()
    } else {
        ScenarioDisplayOptions::compact()
    };
    write_scenarios(writer, scenarios, options)
}

/// Collect the registry, render a listing through `render`, and flush it.
///
/// Every diagnostic writes to stdout and must report a flush failure against
/// the same boundary, so the collection and flush live here once. `render`
/// receives the registry and the stdout handle, which keeps each listing's
/// selection rule directly testable against a buffer.
fn write_listing<F>(render: F) -> Result<()>
where
    F: FnOnce(&RegistryDump, &mut dyn Write) -> Result<()>,
{
    let registry = collect_registry()?;
    let mut stdout = io::stdout();
    render(&registry, &mut stdout)?;
    stdout
        .flush()
        .wrap_err("failed to flush step listing to stdout")
}

/// Write steps selected by `filter`, optionally followed by scenarios.
fn write_filtered_steps<F>(filter: F, scenarios: Option<ScenarioDisplayOptions>) -> Result<()>
where
    F: Fn(&Step) -> bool,
{
    write_listing(|registry, stdout| {
        registry
            .steps
            .iter()
            .filter(|&step| filter(step))
            .try_for_each(|step| write_step(stdout, step))?;
        if let Some(options) = scenarios {
            write_scenarios(stdout, &registry.scenarios, options)?;
        }
        Ok(())
    })
}

/// Serialize skip reports as newline-terminated JSON.
fn write_skip_reports_json(writer: &mut dyn Write, reports: &[SkipReport<'_>]) -> Result<()> {
    serde_json::to_writer(&mut *writer, reports)
        .wrap_err("failed to serialize skip diagnostics to JSON")?;
    writer
        .write_all(b"\n")
        .wrap_err("failed to terminate JSON output with newline")
}

#[cfg(test)]
mod tests;
