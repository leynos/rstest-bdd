//! Command-line diagnostic tooling for rstest-bdd.

use std::collections::HashMap;
use std::path::PathBuf;
use std::process::Command;

use cargo_metadata::Message;
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
fn handle_steps() -> Result<()> {
    for step in collect_steps()? {
        print_step(&step);
    }
    Ok(())
}

/// Handle the `unused` subcommand by listing steps that were never executed.
///
/// # Errors
///
/// Returns an error if the test binaries cannot be built or executed.
fn handle_unused() -> Result<()> {
    for step in collect_steps()?.into_iter().filter(|s| !s.used) {
        print_step(&step);
    }
    Ok(())
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

/// Attempt to extract the test executable path from a Cargo JSON message line.
///
/// The line is parsed as a [`Message`]. If it represents a compiler
/// artifact for a test target and an executable was produced, the path to
/// that executable is returned. Any failure to parse the line or match the
/// criteria results in `None`.
///
/// # Examples
///
/// ```ignore
/// let line = r#"{
///     "reason": "compiler-artifact",
///     "executable": "target/debug/my_test",
///     "target": { "kind": ["test"] }
/// }"#;
/// assert!(extract_test_executable(line).is_some());
/// ```
fn extract_test_executable(line: &str) -> Option<PathBuf> {
    let message = serde_json::from_str::<Message>(line).ok()?;
    if let Message::CompilerArtifact(artifact) = message
        && artifact.target.kind.iter().any(|k| k == "test")
    {
        return artifact.executable.map(|p| p.into());
    }
    None
}

fn collect_steps() -> Result<Vec<Step>> {
    let metadata = cargo_metadata::MetadataCommand::new().exec()?;
    let has_tests = metadata
        .packages
        .iter()
        .flat_map(|p| &p.targets)
        .any(|t| t.kind.iter().any(|k| k == "test"));
    if !has_tests {
        return Ok(Vec::new());
    }

    let output = Command::new("cargo")
        .args(["test", "--no-run", "--message-format=json"])
        .output()
        .wrap_err("failed to build tests")?;
    if !output.status.success() {
        bail!("cargo test failed");
    }

    let mut bins = Vec::new();
    for line in String::from_utf8_lossy(&output.stdout).lines() {
        if let Some(exe) = extract_test_executable(line) {
            bins.push(exe);
        }
    }
    if bins.is_empty() {
        return Ok(Vec::new());
    }

    let mut steps = Vec::new();
    for bin in bins {
        let out = Command::new(&bin)
            .arg("--dump-steps")
            .output()
            .with_context(|| format!("failed to run test binary {}", bin.display()))?;
        if !out.status.success() {
            let err = String::from_utf8_lossy(&out.stderr);
            if err.contains("Unrecognized option: 'dump-steps'") {
                continue;
            }
            bail!("test binary {} failed: {err}", bin.display());
        }
        let mut parsed: Vec<Step> = serde_json::from_slice(&out.stdout)
            .with_context(|| format!("invalid JSON from {}", bin.display()))?;
        steps.append(&mut parsed);
    }
    Ok(steps)
}

/// Print a step definition in diagnostic output.
///
/// # Examples
///
/// ```ignore
/// use cargo_bdd::Step;
///
/// let step = Step {
///     keyword: "Given".into(),
///     pattern: "example".into(),
///     file: "src/example.rs".into(),
///     line: 42,
///     used: false,
/// };
/// print_step(&step);
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
        let line = r#"{
            "reason": "compiler-artifact",
            "executable": "/tmp/test",
            "target": { "kind": ["lib"] }
        }"#;
        assert!(extract_test_executable(line).is_none());
    }
}
