//! Nested-`cargo` process isolation for test-support code.
//!
//! Tests that spawn `cargo` from inside a coverage run inherit an environment
//! that can hang the child: `CARGO_MAKEFLAGS` carries a jobserver the nested
//! process cannot read, and `cargo-llvm-cov`'s control variables redirect the
//! child's build and profile output into the coverage driver's own machinery.
//! Both failure modes look identical to a hang; they are the reason
//! `cargo-bdd::cli::list_steps_runs` timed out under
//! `cargo llvm-cov nextest`.
//!
//! The environment rules live in the `environment` submodule: they are one
//! implementation, shared by every nested `cargo` caller, and they include
//! resolving a relative `CARGO_TARGET_DIR` to an absolute path once, before
//! the value reaches a child or a preparation stamp.
//!
//! Commands are built with [`std::process::Command::env_clear`] and resolved
//! through the `CARGO` compile-time variable so the child never resolves
//! `cargo` via a `PATH` lookup. Execution is bounded in the `bounded`
//! submodule: both output pipes are drained concurrently, the child is killed
//! on timeout, and the error carries the command context plus the captured
//! output.
//!
//! Cold fixture builds are moved out of timed test bodies by the `preparation`
//! submodule, which stamps prepared artefact directories and fails fast under
//! nextest when the named preparation test has not run.

use std::{
    path::Path,
    process::{Command, Output},
};

mod bounded;
mod drain;
mod environment;
mod preparation;
mod profile;

pub use bounded::{run_bounded, run_bounded_with_timeout};
pub use environment::{
    NestedCargoEnv,
    absolute_target_directory,
    build_child_env,
    describe_env,
    env_from_vars,
};
pub use preparation::{
    DEFAULT_PROFILE,
    DEPENDENCY_ARTEFACT_DIRS,
    MAX_SEQUENTIAL_COMMANDS,
    PREPARATION_COMMAND_TIMEOUT,
    Preparation,
    PreparationSummary,
    Sources,
    cargo_config_inputs,
    count_compiling,
    ensure_prepared,
    ensure_prepared_with,
    input_hash,
    prepare_now,
    running_under_nextest,
};
pub use profile::ProfileDestination;

/// Build a `cargo` invocation with the filtered environment.
///
/// The binary is resolved from the `CARGO` compile-time variable (never a
/// `PATH` lookup) and the command starts from an empty environment.
#[must_use]
pub fn cargo_command(env: &NestedCargoEnv, cwd: &Path) -> Command {
    let mut cmd = Command::new(env!("CARGO"));
    cmd.current_dir(cwd);
    env.apply_to(&mut cmd);
    cmd
}

/// Build a command for an arbitrary program with the filtered environment.
///
/// Used by tests that spawn the binary under test (which itself spawns nested
/// `cargo`) with the same isolation the nested-Cargo helper applies, so the
/// binary and its Cargo children never see stale jobserver or coverage
/// control variables.
#[must_use]
pub fn filtered_command(program: &Path, env: &NestedCargoEnv, cwd: &Path) -> Command {
    let mut cmd = Command::new(program);
    cmd.current_dir(cwd);
    env.apply_to(&mut cmd);
    cmd
}

/// Captured output of one bounded nested invocation.
#[derive(Debug, Clone)]
pub struct CapturedOutput {
    /// Whether the child exited successfully.
    pub status: bool,
    /// stdout, decoded lossily.
    pub stdout: String,
    /// stderr, decoded lossily.
    pub stderr: String,
}

impl CapturedOutput {
    /// Combined stdout and stderr, for failure messages.
    #[must_use]
    pub fn combined(&self) -> String {
        format!("stdout:\n{}\nstderr:\n{}", self.stdout, self.stderr)
    }
}

impl From<Output> for CapturedOutput {
    fn from(output: Output) -> Self {
        Self {
            status: output.status.success(),
            stdout: String::from_utf8_lossy(&output.stdout).into_owned(),
            stderr: String::from_utf8_lossy(&output.stderr).into_owned(),
        }
    }
}

impl std::fmt::Display for CapturedOutput {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(&self.combined())
    }
}
