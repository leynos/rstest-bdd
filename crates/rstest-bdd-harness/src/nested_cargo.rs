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
    Admission,
    DEFAULT_PROFILE,
    Deadline,
    Directories,
    Instruments,
    MAX_SEQUENTIAL_COMMANDS,
    PREPARATION_COMMAND_TIMEOUT,
    Preparation,
    PreparationOutcome,
    PreparationSummary,
    Sources,
    UnreadableStamp,
    admit,
    admit_with_probe,
    cargo_config_inputs,
    ensure_prepared_building,
    ensure_prepared_building_reported,
    env_for_directory,
    input_hash,
    instruments_within,
    invalidate,
    invalidate_reported,
    prepare_now,
    prepare_now_reported,
};
pub use profile::ProfileDestination;

/// Count `Compiling` lines in captured child output.
///
/// Preparation summaries report this so the log line shows how much cold work
/// a family paid for. `cargo check` prints `Checking` instead, which is why the
/// count is deliberately of builds and not of units.
///
/// ANSI escape sequences are stripped before matching. CI sets
/// `CARGO_TERM_COLOR=always`, so a nested `cargo` that is not given
/// `--color=never` prefixes each status line with an escape sequence; without
/// the strip, a cold preparation would report zero compilations and the
/// prepared-fixture gate would read that as reuse.
#[must_use]
pub fn count_compiling(stderr: &[u8]) -> usize {
    String::from_utf8_lossy(stderr)
        .lines()
        .filter(|line| strip_ansi(line).trim_start().starts_with("Compiling "))
        .count()
}

/// Remove ANSI escape sequences from one line of captured output.
///
/// Handles both the CSI form Cargo emits for colour (`ESC [ ... m`) and the
/// bare `ESC`-introduced forms, so the result is the visible text.
fn strip_ansi(line: &str) -> String {
    let mut visible = String::with_capacity(line.len());
    let mut chars = line.chars().peekable();
    while let Some(character) = chars.next() {
        if character == ESC {
            skip_escape_sequence(&mut chars);
            continue;
        }
        visible.push(character);
    }
    visible
}

/// The escape character that introduces every sequence [`strip_ansi`] removes.
const ESC: char = '\u{1b}';

/// The last character of a CSI sequence's parameters.
///
/// A CSI sequence is `ESC [`, then parameter bytes, then one character in this
/// range — a letter such as `m`, or one of the punctuation marks Cargo uses.
/// [`skip_escape_sequence`] consumes up to and including the first one it sees.
const CSI_FINAL_BYTE: std::ops::RangeInclusive<char> = '\u{40}'..='\u{7e}';

/// Consume the rest of an escape sequence whose `ESC` was just read.
///
/// The introducer is optional: a lone `ESC` is dropped with nothing to skip,
/// which is why the caller routes every escape through here regardless of what
/// follows it. Parameters are consumed without inspection — colour and cursor
/// sequences differ in them, and both end at the same first final byte — so one
/// loop serves every CSI form Cargo emits.
fn skip_escape_sequence(chars: &mut std::iter::Peekable<std::str::Chars<'_>>) {
    if chars.peek() != Some(&'[') {
        return;
    }
    chars.next();
    for byte in chars.by_ref() {
        if CSI_FINAL_BYTE.contains(&byte) {
            return;
        }
    }
}

/// Whether the current process was started by `cargo nextest`.
///
/// Nextest runs every test in its own process, so preparation cannot be shared
/// lazily within one binary; this decides whether a missing stamp fails fast
/// instead.
#[must_use]
pub fn running_under_nextest() -> bool { std::env::var_os("NEXTEST_RUN_ID").is_some() }

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
