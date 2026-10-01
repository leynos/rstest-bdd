//! Child-process handling for the nested-cargo harness.
//!
//! Fixture-specific commands compose the shared `nested_cargo` process helper
//! with the rebuild experiment's target directory and plain-text Cargo output.

use std::{
    path::Path,
    process::{Command, Output},
};

use rstest_bdd_harness::nested_cargo::{self, NestedCargoEnv, ProfileDestination};
use serde_json::Value;

/// Captures the one deterministic nested-Cargo environment each experiment
/// invocation shares.
pub(crate) type ChildEnv = NestedCargoEnv;

/// Capture a child environment whose incidental coverage stays outside the
/// parent coverage driver's profile destination.
pub(crate) fn build_child_env() -> ChildEnv {
    nested_cargo::build_child_env(
        &super::fixtures::shared_target_dir(),
        ProfileDestination::ChildScratch,
    )
}

/// Build a nested Cargo invocation using the shared environment isolation.
pub(crate) fn cargo_command(env: &ChildEnv, cwd: &Path) -> Command {
    let mut command = nested_cargo::cargo_command(env, cwd);
    // CI can force colour, while this harness matches Cargo's plain-text
    // status lines.
    command.args(["--color", "never"]);
    command
}

/// Describe the resolved child environment in failure diagnostics.
pub(crate) fn describe_env(env: &ChildEnv) -> String { nested_cargo::describe_env(env) }

/// Combined output of one nested invocation.
pub(crate) struct Captured {
    /// Whether the child exited successfully.
    pub(crate) status: bool,
    /// Combined stdout and stderr, decoded lossily.
    pub(crate) stdout: String,
}

impl From<Output> for Captured {
    fn from(output: Output) -> Self {
        Self {
            status: output.status.success(),
            stdout: format!(
                "{}\n{}",
                String::from_utf8_lossy(&output.stdout),
                String::from_utf8_lossy(&output.stderr)
            ),
        }
    }
}

/// Run a nested Cargo command under the shared bounded process runner.
pub(crate) fn run_bounded(command: &mut Command) -> std::io::Result<Output> {
    nested_cargo::run_bounded(command)
}

/// Locate the fixture's test-binary executable in cargo's JSON messages.
///
/// The artefact is located through `--message-format=json` rather than a glob
/// over `<target>/debug/deps/invalidation-*.d`: that directory already holds
/// multiple hashes of `librstest_bdd`, and a glob could match a stale artefact
/// from an earlier run.
pub(crate) fn locate_test_executable(json: &str) -> Option<String> {
    json.lines().find_map(|line| {
        let value: Value = serde_json::from_str(line).ok()?;
        let target = value.get("target")?;
        if target.get("name").and_then(Value::as_str) != Some("invalidation") {
            return None;
        }
        let kinds = target.get("kind")?.as_array()?;
        if !kinds.iter().any(|kind| kind == "bin" || kind == "test") {
            return None;
        }
        value.get("executable")?.as_str().map(str::to_owned)
    })
}
