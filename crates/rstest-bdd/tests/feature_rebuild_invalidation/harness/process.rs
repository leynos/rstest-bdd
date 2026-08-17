//! Child-process handling for the nested-cargo harness.
//!
//! Builds the byte-identical environment every invocation shares, invokes
//! `env!("CARGO")` (never a PATH lookup), imposes the harness's own
//! wall-clock bound on the child, and drains both pipes concurrently while
//! polling for exit — a voluminous build (a cold `--message-format=json` run
//! reports every fresh unit) would otherwise fill the pipe buffer and
//! deadlock the child on a blocking write, making it look hung until the
//! wall-clock bound fires.
//!
//! Environment hygiene is the single most likely spurious-red cause: a
//! rebuild triggered by an environment difference between run 1 and run 2 of
//! the rebuild experiment would fail the test for the wrong reason. The child
//! properties are therefore inherited wholesale from the parent and only
//! surgically adjusted — `CARGO_MAKEFLAGS` (a jobserver a nested process
//! cannot open, a classic Windows stall) and `CARGO_PKG_*` (they describe a
//! different crate) are stripped, `CARGO_TARGET_DIR` falls back to the shared
//! workspace `target/`, and `LLVM_PROFILE_FILE` is redirected to a scratch
//! directory so nested coverage never merges into the parent's gated profile.

use serde_json::Value;
use std::io;
use std::path::Path;
use std::process::{Command, Output, Stdio};
use std::thread;
use std::time::{Duration, Instant};

/// Per-invocation wall-clock bound for each nested `cargo` call. Realistic
/// warm invocations finish in well under 60 s even on a two-core runner; this
/// bound exists to turn a hang into a named, readable failure rather than
/// letting nextest's slow-timeout kill the test with a runner-generated
/// message.
const CHILD_TIMEOUT: Duration = Duration::from_secs(300);

/// The environment applied to every nested `cargo` invocation, built once so
/// run 1 and run 2 of the rebuild experiment are byte-identical.
pub(crate) struct ChildEnv {
    pub(crate) vars: Vec<(std::ffi::OsString, std::ffi::OsString)>,
}

/// Capture a snapshot of the parent environment, adjusted for the nested
/// build. Every invocation of the child uses this exact snapshot.
pub(crate) fn build_child_env() -> ChildEnv {
    let mut vars: Vec<(std::ffi::OsString, std::ffi::OsString)> = std::env::vars_os().collect();
    vars.retain(|(key, _)| {
        let key = key.to_string_lossy();
        key != "CARGO_MAKEFLAGS" && !key.starts_with("CARGO_PKG_")
    });

    // Propagate the shared build directory: whatever Cargo already set (CI's
    // coverage target dir) wins, so the nested build is warm; otherwise fall
    // back to the workspace target.
    if !vars
        .iter()
        .any(|(key, _)| key.to_string_lossy() == "CARGO_TARGET_DIR")
    {
        vars.push((
            "CARGO_TARGET_DIR".into(),
            super::fixtures::shared_target_dir().into_os_string(),
        ));
    }

    // Redirect nested coverage output away from the parent's gated profile so
    // the child's `.profraw` never merges into the parent's pattern.
    if std::env::var_os("LLVM_PROFILE_FILE").is_some() {
        let coverage_dir = super::fixtures::scratch_root().join("coverage");
        if let Err(err) = std::fs::create_dir_all(&coverage_dir) {
            panic!(
                "cannot create scratch coverage dir {}: {err}",
                coverage_dir.display()
            );
        }
        vars.retain(|(key, _)| key.to_string_lossy() != "LLVM_PROFILE_FILE");
        vars.push((
            "LLVM_PROFILE_FILE".into(),
            coverage_dir.join("nested-%p.profraw").into_os_string(),
        ));
    }

    vars.sort_by(|a, b| a.0.cmp(&b.0));
    ChildEnv { vars }
}

/// Build a `cargo` invocation against `.env_clear()` plus the shared child
/// environment, so every invocation is byte-identical.
pub(crate) fn cargo_command(env: &ChildEnv, cwd: &Path) -> Command {
    let mut cmd = Command::new(env!("CARGO"));
    cmd.current_dir(cwd).env_clear();
    for (key, value) in &env.vars {
        cmd.env(key, value);
    }
    cmd
}

/// The resolved child environment, for inclusion in failure messages.
pub(crate) fn describe_env(env: &ChildEnv) -> String {
    env.vars
        .iter()
        .map(|(key, value)| format!("{}={}", key.to_string_lossy(), value.to_string_lossy()))
        .collect::<Vec<_>>()
        .join("\n")
}

/// Combined output of one nested invocation.
pub(crate) struct Captured {
    /// Whether the child exited successfully.
    pub(crate) status: bool,
    /// Combined stdout and stderr, decoded lossily.
    pub(crate) stdout: String,
}

impl From<Output> for Captured {
    fn from(output: Output) -> Self {
        let stdout = format!(
            "{}\n{}",
            String::from_utf8_lossy(&output.stdout),
            String::from_utf8_lossy(&output.stderr)
        );
        Self {
            status: output.status.success(),
            stdout,
        }
    }
}

/// Run a child process to completion, killing it with a clear message if it
/// exceeds `CHILD_TIMEOUT`, and return its output.
pub(crate) fn run_bounded(cmd: &mut Command) -> io::Result<Output> {
    let mut child = cmd.stdout(Stdio::piped()).stderr(Stdio::piped()).spawn()?;
    let Some(stdout) = child.stdout.take() else {
        panic!("piped stdout must be present; run_bounded always pipes");
    };
    let stdout = drain_pipe(stdout);
    let Some(stderr) = child.stderr.take() else {
        panic!("piped stderr must be present; run_bounded always pipes");
    };
    let stderr = drain_pipe(stderr);

    let deadline = Instant::now() + CHILD_TIMEOUT;
    loop {
        match child.try_wait() {
            Ok(Some(_)) => break,
            Ok(None) => {
                if Instant::now() >= deadline {
                    let _ = child.kill();
                    let _ = child.wait();
                    return Err(timeout_error(
                        &join_or_panic(stdout, "stdout"),
                        &join_or_panic(stderr, "stderr"),
                    ));
                }
            }
            Err(err) => return Err(err),
        }
        thread::sleep(Duration::from_millis(50));
    }

    let status = child.wait()?;
    let stdout = join_or_panic(stdout, "stdout");
    let stderr = join_or_panic(stderr, "stderr");
    Ok(Output {
        status,
        stdout,
        stderr,
    })
}

/// Spawn a thread that reads a pipe to EOF and returns its bytes.
fn drain_pipe(pipe: impl io::Read + Send + 'static) -> thread::JoinHandle<Vec<u8>> {
    thread::spawn(move || {
        let mut buf = Vec::new();
        let mut pipe = pipe;
        let _ = pipe.read_to_end(&mut buf);
        buf
    })
}

fn join_or_panic(handle: thread::JoinHandle<Vec<u8>>, what: &str) -> Vec<u8> {
    match handle.join() {
        Ok(bytes) => bytes,
        Err(error) => panic!("{what} reader thread panicked: {error:?}"),
    }
}

fn timeout_error(stdout: &[u8], stderr: &[u8]) -> io::Error {
    io::Error::new(
        io::ErrorKind::TimedOut,
        format!(
            "nested cargo exceeded its {} s wall-clock bound; the child was \
             killed\nstdout so far:\n{}\nstderr so far:\n{}",
            CHILD_TIMEOUT.as_secs(),
            String::from_utf8_lossy(stdout),
            String::from_utf8_lossy(stderr)
        ),
    )
}

/// Locate the fixture's test-binary executable in cargo's JSON messages.
///
/// The artefact is located through `--message-format=json` rather than a glob
/// over `<target>/debug/deps/invalidation-*.d`: that directory already holds
/// over a thousand `.d` files and multiple hashes of `librstest_bdd`, and a
/// glob could match a stale artefact from an earlier (including pre-fix) run.
pub(crate) fn locate_test_executable(json: &str) -> Option<String> {
    json.lines().find_map(|line| {
        let value: Value = serde_json::from_str(line).ok()?;
        // `compiler-artifact` is the only cargo JSON reason that carries an
        // `executable`, so checking reason equality is redundant; the fields
        // below fully determine the message (and keep the source free of a
        // spelling-gate dupe of the upstream US term).
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
