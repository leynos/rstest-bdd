//! Registry collection helpers shared by the CLI subcommands.

use std::{
    collections::HashSet,
    io::{self, BufReader, Read, Write},
    path::{Path, PathBuf},
    process::{Child, Command, Stdio},
};

use cargo_metadata::{Message, Package, PackageId, Target, TargetKind};
use eyre::{Context, Result, bail, eyre};
use serde::Deserialize;
use tracing::warn;

/// Registry step entry including location metadata and execution status.
#[derive(Debug, Deserialize, Clone)]
pub(crate) struct Step {
    #[serde(default = "default_global_library")]
    /// Stable library identity that owns the step.
    pub library: String,
    /// Gherkin keyword for the step.
    pub keyword: String,
    /// Registered step pattern.
    pub pattern: String,
    /// Source file containing the step definition.
    pub file: String,
    /// Source line containing the step definition.
    pub line: u32,
    /// Whether the step was executed by a scenario.
    pub used: bool,
}

/// Step definition that was bypassed when a scenario requested a skip.
#[derive(Debug, Deserialize, Clone)]
pub(crate) struct BypassedStep {
    #[serde(default = "default_global_library")]
    /// Stable library identity that owns the bypassed step.
    pub library: String,
    /// Gherkin keyword for the step.
    pub keyword: String,
    /// Registered step pattern.
    pub pattern: String,
    /// Source file containing the step definition.
    pub file: String,
    /// Source line containing the step definition.
    pub line: u32,
    /// Feature containing the skipped scenario.
    pub feature_path: String,
    /// Name of the skipped scenario.
    pub scenario_name: String,
    #[serde(default)]
    /// Source line of the skipped scenario.
    pub scenario_line: u32,
    #[serde(default)]
    /// Tags attached to the skipped scenario.
    pub tags: Vec<String>,
    #[serde(default = "default_global_libraries")]
    /// Closed library identities selected by the skipped scenario.
    pub libraries: Vec<String>,
    /// Optional reason recorded for the skip.
    pub reason: Option<String>,
}

/// Scenario outcome labels.
#[derive(Debug, Deserialize, Clone, Copy, PartialEq, Eq)]
#[serde(rename_all = "lowercase")]
pub(crate) enum ScenarioOutcome {
    /// The scenario completed successfully.
    Passed,
    /// The scenario was skipped.
    Skipped,
}

/// Registry scenario entry including metadata and skip flags.
#[derive(Debug, Deserialize, Clone)]
pub(crate) struct Scenario {
    /// Feature containing the scenario.
    pub feature_path: String,
    #[serde(rename = "scenario_name")]
    /// Scenario name.
    pub name: String,
    /// Scenario execution outcome.
    pub status: ScenarioOutcome,
    /// Optional execution message.
    pub message: Option<String>,
    /// Whether the scenario may be skipped.
    pub allow_skipped: bool,
    /// Whether the scenario was forced to fail.
    pub forced_failure: bool,
    #[serde(default)]
    /// Source line of the scenario.
    pub line: u32,
    #[serde(default)]
    /// Tags attached to the scenario.
    pub tags: Vec<String>,
    #[serde(default = "default_global_libraries")]
    /// Closed library identities selected by the scenario.
    pub libraries: Vec<String>,
}

/// Aggregated registry export holding every collected step and scenario from a
/// test run; serde defaults ensure absent collections deserialize as empty
/// vectors to simplify merges.
#[derive(Debug, Deserialize, Clone, Default)]
#[serde(default)]
pub(crate) struct RegistryDump {
    /// Registered step definitions.
    pub steps: Vec<Step>,
    /// Collected scenario outcomes.
    pub scenarios: Vec<Scenario>,
    /// Steps bypassed by skipped scenarios.
    pub bypassed_steps: Vec<BypassedStep>,
}

impl RegistryDump {
    /// Append the entries from another registry dump.
    pub(crate) fn merge(&mut self, mut other: Self) {
        self.steps.append(&mut other.steps);
        self.scenarios.append(&mut other.scenarios);
        self.bypassed_steps.append(&mut other.bypassed_steps);
    }
}

/// Build the workspace tests and merge their registry dumps.
pub(crate) fn collect_registry() -> Result<RegistryDump> {
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

/// Return whether workspace metadata contains at least one test target.
fn has_test_targets(metadata: &cargo_metadata::Metadata) -> bool {
    metadata.packages.iter().any(|p| {
        p.targets
            .iter()
            .any(|target| target.kind.iter().any(|kind| kind == &TargetKind::Test))
    })
}

/// Build workspace test targets and return their executable paths.
fn build_test_binaries(metadata: &cargo_metadata::Metadata) -> Result<Vec<PathBuf>> {
    let workspace: HashSet<_> = metadata.workspace_members.iter().collect();
    let mut bins = Vec::new();
    let mut seen = HashSet::new();
    for package in workspace_packages(&metadata.packages, &workspace) {
        collect_package_binaries(package, &mut bins, &mut seen)?;
    }
    bins.sort_by(|a, b| a.as_os_str().cmp(b.as_os_str()));
    Ok(bins)
}

/// Build all test binaries for one workspace package.
fn collect_package_binaries(
    package: &Package,
    bins: &mut Vec<PathBuf>,
    seen: &mut HashSet<PathBuf>,
) -> Result<()> {
    for target in test_targets(&package.targets) {
        let extracted = build_test_target(package, target)?;
        for bin in extracted {
            if seen.contains(&bin) {
                continue;
            }
            seen.insert(bin.clone());
            bins.push(bin);
        }
    }
    Ok(())
}

/// Select packages belonging to the workspace.
fn workspace_packages<'a>(
    packages: &'a [Package],
    workspace: &'a HashSet<&'a PackageId>,
) -> impl Iterator<Item = &'a Package> + 'a {
    packages.iter().filter(move |p| workspace.contains(&p.id))
}

/// Select test targets from a package's targets.
fn test_targets(targets: &[Target]) -> impl Iterator<Item = &Target> + '_ {
    targets
        .iter()
        .filter(|target| target.kind.iter().any(|kind| kind == &TargetKind::Test))
}

/// Parse Cargo's JSON messages and collect test executable paths.
fn parse_cargo_messages(
    reader: BufReader<impl Read>,
    child: &mut Child,
    package_name: &str,
    target_name: &str,
) -> Result<Vec<PathBuf>> {
    let mut bins = Vec::new();
    for message in Message::parse_stream(reader) {
        let message = match message {
            Ok(message) => message,
            Err(err) => {
                let _ = child.kill();
                let _ = child.wait();
                return Err(eyre!(err)).wrap_err_with(|| {
                    format!(
                        "failed to parse cargo metadata message for target {target_name} in \
                         package {package_name}",
                    )
                });
            }
        };
        if let Some(exe) = extract_test_executable(&message) {
            bins.push(exe);
        }
    }
    Ok(bins)
}

/// Report a failed test build and allow registry collection to continue.
fn handle_build_failure(package_name: &str, target_name: &str) -> Result<()> {
    let mut stderr = io::stderr();
    writeln!(
        &mut stderr,
        "warning: cargo test failed for target {target_name} in package {package_name}; skipping",
    )
    .wrap_err("failed to write warning to stderr")
}

/// Build one test target and extract its executable path.
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
    let stdout = child.stdout.take().ok_or_else(|| {
        eyre!(
            "cargo test did not provide stdout for target {} in package {}",
            target.name,
            package.name
        )
    })?;
    let reader = BufReader::new(stdout);
    let bins = parse_cargo_messages(reader, &mut child, &package.name, &target.name)?;
    let status = child.wait().wrap_err_with(|| {
        format!(
            "cargo test failed for target {} in package {}",
            target.name, package.name
        )
    })?;
    if !status.success() {
        handle_build_failure(&package.name, &target.name)?;
        return Ok(Vec::new());
    }
    Ok(bins)
}

/// Run a test binary and parse its registry dump, if supported.
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

/// Deserialize a registry dump while ignoring unknown fields.
fn parse_registry_dump(bytes: &[u8]) -> serde_json::Result<RegistryDump> {
    // Deliberately lenient: unknown fields are ignored so newer rstest-bdd
    // versions can extend the registry dump schema without breaking older
    // cargo-bdd consumers.
    serde_json::from_slice(bytes)
}

/// Handle a failed test binary invocation.
fn handle_binary_execution_failure(
    bin: &Path,
    output: &std::process::Output,
) -> Result<Option<RegistryDump>> {
    let err = String::from_utf8_lossy(&output.stderr);
    handle_binary_execution_stderr(bin, &err)
}

/// Interpret test-binary stderr and report unsupported dump flags.
fn handle_binary_execution_stderr(bin: &Path, stderr: &str) -> Result<Option<RegistryDump>> {
    if is_unrecognized_dump_steps(stderr) {
        warn!(
            binary = %bin.file_name().unwrap_or_default().to_string_lossy(),
            reason = "unsupported-dump-steps",
            "registry collection skipped the binary"
        );
        Ok(None)
    } else {
        bail!("test binary {} failed: {stderr}", bin.display());
    }
}

/// Determine whether stderr output indicates the test binary does not
/// recognize the `--dump-steps` flag.
pub(crate) fn is_unrecognized_dump_steps(stderr: &str) -> bool {
    let lower = stderr.to_ascii_lowercase();
    lower.lines().any(|line| {
        let mentions_flag = line.contains("--dump-steps") || line.contains("'dump-steps'");
        mentions_flag
            && [
                "unrecognized option",
                "wasn't expected",
                "unknown option",
                "invalid option",
                "unexpected argument",
            ]
            .iter()
            .any(|pattern| line.contains(pattern))
    })
}

/// Attempt to extract the test executable path from a Cargo message.
pub(crate) fn extract_test_executable(msg: &Message) -> Option<PathBuf> {
    match msg {
        Message::CompilerArtifact(artefact)
            if artefact
                .target
                .kind
                .iter()
                .any(|kind| kind == &TargetKind::Test) =>
        {
            artefact.executable.clone().map(PathBuf::from)
        }
        _ => None,
    }
}

#[cfg(test)]
mod tests;

/// Stable identity assigned to steps from legacy unscoped suites.
const GLOBAL_LIBRARY: &str = "rstest_bdd::global";

/// Supply the global identity when reading an older step dump.
fn default_global_library() -> String { GLOBAL_LIBRARY.to_owned() }

/// Supply the global scope when reading older scenario and bypass dumps.
fn default_global_libraries() -> Vec<String> { vec![default_global_library()] }
