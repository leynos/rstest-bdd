//! Command line diagnostic tooling for rstest-bdd.

use std::collections::HashMap;
use std::io::Write;
use std::io::{self, BufReader};
use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};

use cargo_metadata::{Message, Package, PackageId, Target};
use clap::{Parser, Subcommand};
use eyre::{bail, eyre, Context, Result};
use serde::Deserialize;

mod output;
use output::{write_group_separator, write_scenarios, write_step};

/// Cargo subcommand providing diagnostics for rstest-bdd.
#[derive(Parser)]
#[command(author, version, about)]
struct Cli {
    #[command(subcommand)]
    command: Commands,
}

/// Supported diagnostic commands.
#[derive(Subcommand)]
enum Commands {
    /// List all registered steps.
    Steps,
    /// List registered steps that were never executed.
    Unused,
    /// List step definitions that share the same keyword and pattern.
    Duplicates,
}

#[derive(Debug, Deserialize, Clone)]
struct Step {
    keyword: String,
    pattern: String,
    file: String,
    line: u32,
    used: bool,
}

#[derive(Debug, Deserialize, Clone, Default)]
struct RegistryDump {
    steps: Vec<Step>,
    scenarios: Vec<Scenario>,
}

impl RegistryDump {
    fn merge(&mut self, mut other: Self) {
        self.steps.append(&mut other.steps);
        self.scenarios.append(&mut other.scenarios);
    }
}

#[derive(Debug, Deserialize, Clone, Copy, PartialEq, Eq)]
#[serde(rename_all = "lowercase")]
enum ScenarioOutcome {
    Passed,
    Skipped,
}

#[derive(Debug, Deserialize, Clone)]
struct Scenario {
    feature_path: String,
    #[serde(rename = "scenario_name")]
    name: String,
    status: ScenarioOutcome,
    message: Option<String>,
    allow_skipped: bool,
    forced_failure: bool,
}

fn main() -> Result<()> {
    match Cli::parse().command {
        Commands::Steps => handle_steps()?,
        Commands::Unused => handle_unused()?,
        Commands::Duplicates => handle_duplicates()?,
    }
    Ok(())
}

/// Handle the `steps` subcommand by listing all registered steps.
///
/// Write filtered steps to stdout and optionally append scenario summaries.
///
/// This helper encapsulates the shared plumbing around registry collection,
/// filtering, output rendering, and flushing so that subcommands can focus on
/// their filtering semantics.
///
/// # Examples
///
/// ```ignore
/// // Emit every registered step along with their scenarios.
/// write_filtered_steps(|_| true, true)?;
///
/// // Emit only unused steps and omit scenario listings.
/// write_filtered_steps(|step| !step.used, false)?;
/// ```
fn write_filtered_steps<F>(filter: F, include_scenarios: bool) -> Result<()>
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
    if include_scenarios {
        write_scenarios(&mut stdout, &registry.scenarios)?;
    }
    stdout
        .flush()
        .wrap_err("failed to flush step listing to stdout")?;
    Ok(())
}

/// Handle the `steps` subcommand by listing all registered steps.
///
/// # Errors
///
/// Returns an error if the test binaries cannot be built or executed.
fn handle_steps() -> Result<()> {
    write_filtered_steps(|_| true, true)
}

/// Handle the `unused` subcommand by listing steps that were never executed.
///
/// # Errors
///
/// Returns an error if the test binaries cannot be built or executed.
fn handle_unused() -> Result<()> {
    write_filtered_steps(|step| !step.used, false)
}

/// Handle the `duplicates` subcommand by grouping identical step definitions.
///
/// # Errors
///
/// Returns an error if the test binaries cannot be built or executed.
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
        .wrap_err("failed to flush duplicate listing to stdout")?;
    Ok(())
}

/// Attempt to extract the test executable path from a Cargo message.
///
/// If the message describes a compiler artefact for a test target and an
/// executable was produced, the path to that executable is returned. Messages
/// for other artefacts yield `None`.
///
/// # Examples
///
/// ```ignore
/// use cargo_metadata::Message;
///
/// let msg: Message = serde_json::from_str(r#"{
///     "reason": "compiler-artifact",
///     "executable": "target/debug/my_test",
///     "target": { "kind": ["test"] }
/// }"#).unwrap();
/// assert!(extract_test_executable(&msg).is_some());
/// ```
fn extract_test_executable(msg: &Message) -> Option<PathBuf> {
    match msg {
        Message::CompilerArtifact(artifact)
            if artifact.target.kind.iter().any(|kind| kind == "test") =>
        {
            artifact.executable.clone().map(PathBuf::from)
        }
        _ => None,
    }
}

/// Determine whether stderr output indicates the test binary does not
/// recognise the `--dump-steps` flag.
///
/// The check is case-insensitive and matches several common phrases used by
/// argument parsers when an option is unknown.
///
/// # Examples
///
/// ```
/// assert!(is_unrecognised_dump_steps(
///     "error: Unrecognized option '--dump-steps'",
/// ));
/// assert!(is_unrecognised_dump_steps(
///     "error: Found argument '--dump-steps' which wasn't expected",
/// ));
/// assert!(is_unrecognised_dump_steps(
///     "error: Unrecognized option: 'dump-steps'",
/// ));
/// assert!(!is_unrecognised_dump_steps("some other error"));
/// ```
fn is_unrecognised_dump_steps(stderr: &str) -> bool {
    let lower = stderr.to_ascii_lowercase();
    let has_flag = lower.contains("--dump-steps") || lower.contains("'dump-steps'");
    has_flag
        && [
            "unrecognized option",
            "wasn't expected",
            "unknown option",
            "invalid option",
        ]
        .iter()
        .any(|p| lower.contains(p))
}

fn collect_registry() -> Result<RegistryDump> {
    let metadata = cargo_metadata::MetadataCommand::new().exec()?;
    if !has_test_targets(&metadata) {
        return Ok(RegistryDump::default());
    }
    let bins = build_test_binaries(&metadata)?;
    let mut registry = RegistryDump::default();
    for bin in bins {
        if let Some(parsed) = collect_registry_from_binary(&bin)? {
            registry.merge(parsed);
        }
    }
    Ok(registry)
}

fn has_test_targets(metadata: &cargo_metadata::Metadata) -> bool {
    metadata
        .packages
        .iter()
        .any(|p| p.targets.iter().any(|t| t.kind.iter().any(|k| k == "test")))
}

fn build_test_binaries(metadata: &cargo_metadata::Metadata) -> Result<Vec<PathBuf>> {
    let workspace: std::collections::HashSet<_> = metadata.workspace_members.iter().collect();
    let mut bins = Vec::new();
    for package in workspace_packages(&metadata.packages, &workspace) {
        for target in test_targets(&package.targets) {
            let mut extracted = build_test_target(package, target)?;
            bins.append(&mut extracted);
        }
    }
    Ok(bins)
}

fn workspace_packages<'a>(
    packages: &'a [Package],
    workspace: &'a std::collections::HashSet<&'a PackageId>,
) -> impl Iterator<Item = &'a Package> + 'a {
    packages.iter().filter(move |p| workspace.contains(&p.id))
}

fn test_targets(targets: &[Target]) -> impl Iterator<Item = &Target> + '_ {
    targets
        .iter()
        .filter(|t| t.kind.iter().any(|k| k == "test"))
}

fn build_test_target(package: &Package, target: &Target) -> Result<Vec<PathBuf>> {
    let mut cmd = Command::new("cargo");
    cmd.args([
        "test",
        "--no-run",
        "--message-format=json",
        "--all-features", // include optional diagnostics
        "--package",
        &package.name,
        "--test",
        &target.name,
    ]);
    let mut child = cmd.stdout(Stdio::piped()).spawn().with_context(|| {
        format!(
            "failed to build test target {} in package {}",
            target.name, package.name
        )
    })?;
    let stdout = child.stdout.take().ok_or_else(|| {
        eyre!(
            "cargo test did not provide stdout for target {} in package {}",
            target.name,
            package.name
        )
    })?;
    let reader = BufReader::new(stdout);
    let mut bins = Vec::new();
    for m in Message::parse_stream(reader).flatten() {
        if let Some(exe) = extract_test_executable(&m) {
            bins.push(exe);
        }
    }
    let status = child.wait().wrap_err_with(|| {
        format!(
            "cargo test failed for target {} in package {}",
            target.name, package.name
        )
    })?;
    if !status.success() {
        // Ignore failing targets so incompatible crates do not break step discovery
        let mut stderr = io::stderr();
        writeln!(
            &mut stderr,
            "warning: cargo test failed for target {} in package {}; skipping",
            target.name, package.name
        )
        .wrap_err("failed to write warning to stderr")?;
        return Ok(Vec::new());
    }
    Ok(bins)
}

fn collect_registry_from_binary(bin: &Path) -> Result<Option<RegistryDump>> {
    let output = Command::new(bin)
        .arg("--dump-steps")
        .env("RSTEST_BDD_DUMP_STEPS", "1")
        .output()
        .with_context(|| format!("failed to run test binary {}", bin.display()))?;
    if !output.status.success() {
        return handle_binary_execution_failure(bin, &output);
    }
    let dump = parse_registry_dump(&output.stdout)
        .with_context(|| format!("invalid JSON from {}", bin.display()))?;
    Ok(Some(dump))
}

fn parse_registry_dump(bytes: &[u8]) -> serde_json::Result<RegistryDump> {
    serde_json::from_slice(bytes)
}

fn handle_binary_execution_failure(
    bin: &Path,
    output: &std::process::Output,
) -> Result<Option<RegistryDump>> {
    let err = String::from_utf8_lossy(&output.stderr);
    if is_unrecognised_dump_steps(&err) {
        Ok(None)
    } else {
        bail!("test binary {} failed: {err}", bin.display());
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn ignores_non_test_artifacts() {
        let msg = Message::TextLine(String::new());
        assert!(extract_test_executable(&msg).is_none());
    }

    #[test]
    fn recognises_unknown_flag_errors() {
        assert!(is_unrecognised_dump_steps(
            "error: Unrecognized option '--dump-steps'",
        ));
        assert!(is_unrecognised_dump_steps(
            "error: Found argument '--dump-steps' which wasn't expected",
        ));
        assert!(is_unrecognised_dump_steps(
            "error: unknown option '--dump-steps'",
        ));
        assert!(is_unrecognised_dump_steps(
            "error: Unrecognized option: 'dump-steps'",
        ));
        assert!(!is_unrecognised_dump_steps("different failure"));
    }
}
