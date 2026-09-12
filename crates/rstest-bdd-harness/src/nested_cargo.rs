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
//! The environment rules are one implementation, shared by every nested
//! `cargo` caller:
//!
//! * `CARGO_MAKEFLAGS` is removed (stale jobserver).
//! * `CARGO_PKG_*` is removed (describes a different crate).
//! * `CARGO_LLVM_COV` and every `CARGO_LLVM_COV_*` variable is removed (outer coverage driver
//!   control).
//! * an existing `CARGO_TARGET_DIR` is preserved so nested builds reuse the outer build cache;
//! * when absent, the caller's fallback target directory is used;
//! * an inherited `LLVM_PROFILE_FILE` is kept by default, and replaced with a unique child-only
//!   scratch destination when the caller asks for [`ProfileDestination::ChildScratch`], so nested
//!   coverage never merges into the parent's gated profile;
//! * unrelated variables are preserved.
//!
//! Keeping the inherited profile is the default because `cargo llvm-cov` merges
//! the `.profraw` files named by the pattern it exported: replacing that pattern
//! silently drops the coverage of every process spawned with the result, which
//! then reads as untested code rather than as an uncollected profile. The
//! destination itself lives in the private `profile` submodule.
//!
//! Commands are built with [`std::process::Command::env_clear`] and resolved
//! through the `CARGO` compile-time variable so the child never resolves
//! `cargo` via a `PATH` lookup, and execution is bounded: both output pipes
//! are drained concurrently (a chatty build would otherwise fill the pipe
//! buffer and deadlock), the child is killed on timeout, and the error carries
//! the command context plus the captured output.

use std::{
    collections::BTreeMap,
    env,
    ffi::{OsStr, OsString},
    io,
    path::Path,
    process::{Child, Command, Output, Stdio},
    thread,
    time::{Duration, Instant},
};

mod profile;

pub use profile::ProfileDestination;

/// Per-invocation wall-clock bound for each nested `cargo` call.
///
/// Realistic warm invocations finish well inside this bound; it exists to
/// turn a hang into a named failure with the captured output attached rather
/// than letting the test runner's own timeout kill the test with a
/// runner-generated message.
const CHILD_TIMEOUT: Duration = Duration::from_secs(300);

/// Build the deterministic nested-Cargo environment for this process.
///
/// Captures the current environment and applies the shared filtering rules;
/// `fallback_target_dir` is used when the parent has no `CARGO_TARGET_DIR`.
#[must_use]
pub fn build_child_env(
    fallback_target_dir: &Path,
    profile_destination: ProfileDestination,
) -> NestedCargoEnv {
    env_from_vars(
        env::vars_os().collect(),
        Some(fallback_target_dir),
        profile_destination,
    )
}

/// Build a nested-Cargo environment from an explicit variable snapshot.
///
/// Exposed for the workspace's own tests, which assert the filtering rules
/// against synthetic values without mutating the real process environment.
/// Production callers should use [`build_child_env`].
///
/// # Panics
///
/// Panics when `vars` has no `CARGO_TARGET_DIR` and no fallback is supplied:
/// a nested Cargo invocation without a target directory would silently build
/// into the caller's manifest directory.
#[must_use]
pub fn env_from_vars(
    vars: Vec<(OsString, OsString)>,
    fallback_target_dir: Option<&Path>,
    profile_destination: ProfileDestination,
) -> NestedCargoEnv {
    NestedCargoEnv::from_vars(vars, fallback_target_dir, profile_destination)
}

/// The filtered environment applied to every nested `cargo` invocation.
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct NestedCargoEnv(BTreeMap<OsString, OsString>);

impl NestedCargoEnv {
    /// Build the filtered environment from raw variable pairs.
    fn from_vars(
        mut vars: Vec<(OsString, OsString)>,
        fallback_target_dir: Option<&Path>,
        profile_destination: ProfileDestination,
    ) -> Self {
        vars.retain(|(key, _)| {
            let key = key.to_string_lossy();
            key != "CARGO_MAKEFLAGS"
                && !key.starts_with("CARGO_PKG_")
                && !key.starts_with("CARGO_LLVM_COV")
        });

        // Propagate the shared build directory: whatever the parent already
        // set (CI's coverage target dir) wins, so nested builds stay warm;
        // otherwise the caller's fallback applies.
        if !vars
            .iter()
            .any(|(key, _)| key.to_string_lossy() == "CARGO_TARGET_DIR")
        {
            let Some(fallback) = fallback_target_dir else {
                panic!(
                    "no CARGO_TARGET_DIR in the parent environment and no fallback target \
                     directory provided"
                );
            };
            vars.push((
                OsString::from("CARGO_TARGET_DIR"),
                fallback.as_os_str().to_os_string(),
            ));
        }

        // Redirect nested coverage output away from the parent's gated profile
        // so the child's `.profraw` never merges into the parent's pattern. The
        // process under test keeps the inherited pattern instead: its coverage
        // is the measurement, and a scratch path outside the coverage
        // directory loses it silently.
        if profile_destination == ProfileDestination::ChildScratch
            && vars
                .iter()
                .any(|(key, _)| key.to_string_lossy() == "LLVM_PROFILE_FILE")
        {
            vars.retain(|(key, _)| key.to_string_lossy() != "LLVM_PROFILE_FILE");
            vars.push((
                OsString::from("LLVM_PROFILE_FILE"),
                profile::unique_profile_path().into_os_string(),
            ));
        }

        Self(vars.into_iter().collect())
    }

    /// Whether `key` is set in the filtered environment.
    #[must_use]
    pub fn contains_key(&self, key: &str) -> bool { self.0.contains_key(OsStr::new(key)) }

    /// The value of `key` in the filtered environment, if present.
    #[must_use]
    pub fn get(&self, key: &str) -> Option<&OsStr> {
        self.0.get(OsStr::new(key)).map(OsString::as_os_str)
    }

    /// Iterate the filtered environment as sorted `(key, value)` pairs.
    pub fn iter(&self) -> impl Iterator<Item = (&OsString, &OsString)> { self.0.iter() }

    /// Apply the environment to `cmd`, starting from an empty environment.
    pub fn apply_to(&self, cmd: &mut Command) {
        cmd.env_clear();
        for (key, value) in &self.0 {
            cmd.env(key, value);
        }
    }
}

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

/// Describe the filtered environment for inclusion in failure messages.
#[must_use]
pub fn describe_env(env: &NestedCargoEnv) -> String {
    env.0
        .iter()
        .map(|(key, value)| format!("{}={}", key.to_string_lossy(), value.to_string_lossy()))
        .collect::<Vec<_>>()
        .join("\n")
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

/// Run a child process to completion under the wall-clock bound, draining
/// both output pipes concurrently.
///
/// On timeout the child is killed and the error carries the output captured
/// so far.
///
/// # Errors
///
/// Returns the spawn or wait error when the child cannot be spawned, and a
/// [`std::io::ErrorKind::TimedOut`] error carrying the captured output when
/// the bound is exceeded.
pub fn run_bounded(cmd: &mut Command) -> io::Result<Output> {
    run_bounded_with_timeout(cmd, CHILD_TIMEOUT)
}

/// [`run_bounded`] with an explicit timeout, for callers that size their own
/// bound.
///
/// # Errors
///
/// As [`run_bounded`].
pub fn run_bounded_with_timeout(cmd: &mut Command, timeout: Duration) -> io::Result<Output> {
    let mut child = cmd.stdout(Stdio::piped()).stderr(Stdio::piped()).spawn()?;
    let Some(stdout_pipe) = child.stdout.take() else {
        return Err(broken_pipe("stdout"));
    };
    let Some(stderr_pipe) = child.stderr.take() else {
        return Err(broken_pipe("stderr"));
    };
    let stdout = spawn_drainer(stdout_pipe);
    let stderr = spawn_drainer(stderr_pipe);

    match wait_bounded(&mut child, timeout) {
        Outcome::Exited(status) => Ok(Output {
            status,
            stdout: join_or_empty(stdout, "stdout"),
            stderr: join_or_empty(stderr, "stderr"),
        }),
        Outcome::TimedOut => {
            let _ = child.kill();
            let _ = child.wait();
            Err(timeout_error(
                timeout,
                &join_or_empty(stdout, "stdout"),
                &join_or_empty(stderr, "stderr"),
            ))
        }
        Outcome::Failed(err) => {
            let _ = child.kill();
            let _ = child.wait();
            Err(err)
        }
    }
}

/// Result of the bounded wait.
enum Outcome {
    /// The child exited with `status`.
    Exited(std::process::ExitStatus),
    /// The wall-clock bound elapsed before the child exited.
    TimedOut,
    /// Waiting on the child failed.
    Failed(io::Error),
}

/// Poll the child until it exits, the deadline passes, or waiting fails.
fn wait_bounded(child: &mut Child, timeout: Duration) -> Outcome {
    let deadline = Instant::now() + timeout;
    loop {
        match child.try_wait() {
            Ok(Some(status)) => return Outcome::Exited(status),
            Ok(None) => {
                if Instant::now() >= deadline {
                    return Outcome::TimedOut;
                }
            }
            Err(err) => return Outcome::Failed(err),
        }
        thread::sleep(Duration::from_millis(50));
    }
}

/// Error for a missing piped stream; `run_bounded` always pipes both.
fn broken_pipe(stream: &str) -> io::Error {
    io::Error::new(
        io::ErrorKind::UnexpectedEof,
        format!("nested cargo did not provide a piped {stream}; run_bounded always pipes"),
    )
}

/// Drain a pipe to EOF on a background thread.
fn spawn_drainer(pipe: impl std::io::Read + Send + 'static) -> thread::JoinHandle<Vec<u8>> {
    thread::spawn(move || {
        let mut buf = Vec::new();
        let mut pipe = pipe;
        let _ = pipe.read_to_end(&mut buf);
        buf
    })
}

/// Join a drainer thread, returning empty bytes on a panicking reader.
fn join_or_empty(handle: thread::JoinHandle<Vec<u8>>, what: &str) -> Vec<u8> {
    handle.join().unwrap_or_else(|error| {
        tracing::warn!(stream = what, ?error, "nested-cargo output reader panicked");
        Vec::new()
    })
}

/// Build the timeout error with the command context and captured output.
fn timeout_error(timeout: Duration, stdout: &[u8], stderr: &[u8]) -> io::Error {
    io::Error::new(
        io::ErrorKind::TimedOut,
        format!(
            "nested cargo exceeded its {} s wall-clock bound; the child was killed;\nstdout so \
             far:\n{}\nstderr so far:\n{}",
            timeout.as_secs(),
            String::from_utf8_lossy(stdout),
            String::from_utf8_lossy(stderr)
        ),
    )
}
