//! Command line diagnostic tooling for rstest-bdd.

use std::collections::HashMap;
use std::io::BufReader;
use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};

use cargo_metadata::{Message, Package, PackageId, Target};
use clap::{Parser, Subcommand};
use eyre::{Context, Result, bail};
use serde::Deserialize;

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
/// # Errors
///
/// Returns an error if the test binaries cannot be built or executed.
fn list_steps<F>(filter: F) -> Result<()>
where
    F: Fn(&Step) -> bool,
{
    collect_steps()?
        .into_iter()
        .filter(filter)
        .for_each(|s| print_step(&s));
    Ok(())
}

/// Handle the `steps` subcommand by listing all registered steps.
///
/// # Errors
///
/// Returns an error if the test binaries cannot be built or executed.
fn handle_steps() -> Result<()> {
    list_steps(|_| true)
}

/// Handle the `unused` subcommand by listing steps that were never executed.
///
/// # Errors
///
/// Returns an error if the test binaries cannot be built or executed.
fn handle_unused() -> Result<()> {
    list_steps(|s| !s.used)
}

/// Handle the `duplicates` subcommand by grouping identical step definitions.
///
/// # Errors
///
/// Returns an error if the test binaries cannot be built or executed.
fn handle_duplicates() -> Result<()> {
    let mut groups: HashMap<(String, String), Vec<Step>> = HashMap::new();
    for step in collect_steps()? {
        groups
            .entry((step.keyword.clone(), step.pattern.clone()))
            .or_default()
            .push(step);
    }
    for group in groups.into_values().filter(|g| g.len() > 1) {
        for step in &group {
            print_step(step);
        }
        println!("---");
    }
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
    if let Message::CompilerArtifact(artifact) = msg
        && artifact.target.kind.iter().any(|k| k == "test")
    {
        return artifact.executable.clone().map(|p| p.into());
    }
    None
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

fn collect_steps() -> Result<Vec<Step>> {
    let metadata = cargo_metadata::MetadataCommand::new().exec()?;
    if !has_test_targets(&metadata) {
        return Ok(Vec::new());
    }
    let bins = build_test_binaries(&metadata)?;
    let mut steps = Vec::new();
    for bin in bins {
        if let Some(mut parsed) = collect_steps_from_binary(&bin)? {
            steps.append(&mut parsed);
        }
    }
    Ok(steps)
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

fn test_targets<'a>(targets: &'a [Target]) -> impl Iterator<Item = &'a Target> + 'a {
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
        "--all-features",
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
    let reader = BufReader::new(child.stdout.take().expect("stdout"));
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
        bail!(
            "cargo test failed for target {} in package {}",
            target.name,
            package.name
        );
    }
    Ok(bins)
}

fn collect_steps_from_binary(bin: &Path) -> Result<Option<Vec<Step>>> {
    let output = Command::new(bin)
        .arg("--dump-steps")
        .env("RSTEST_BDD_DUMP_STEPS", "1")
        .output()
        .with_context(|| format!("failed to run test binary {}", bin.display()))?;
    if !output.status.success() {
        return handle_binary_execution_failure(bin, &output);
    }
    let steps: Vec<Step> = serde_json::from_slice(&output.stdout)
        .with_context(|| format!("invalid JSON from {}", bin.display()))?;
    Ok(Some(steps))
}

fn handle_binary_execution_failure(
    bin: &Path,
    output: &std::process::Output,
) -> Result<Option<Vec<Step>>> {
    let err = String::from_utf8_lossy(&output.stderr);
    if is_unrecognised_dump_steps(&err) {
        Ok(None)
    } else {
        bail!("test binary {} failed: {err}", bin.display());
    }
}

/// Print a step definition in diagnostic output.
///
/// # Examples
///
/// ```ignore
/// let step = {
///     #[derive(Debug, serde::Deserialize, Clone)]
///     struct Step {
///         keyword: String,
///         pattern: String,
///         file: String,
///         line: u32,
///         used: bool,
///     }
///     Step {
///         keyword: "Given".into(),
///         pattern: "example".into(),
///         file: "src/example.rs".into(),
///         line: 42,
///         used: false,
///     }
/// };
/// print_step(&step); // same formatting as the real type
/// ```
fn print_step(step: &Step) {
    println!(
        "{} '{}' ({}:{})",
        step.keyword, step.pattern, step.file, step.line
    );
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
