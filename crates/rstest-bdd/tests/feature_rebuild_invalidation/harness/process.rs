//! Child-process handling for the nested-cargo harness.
//!
//! Builds the byte-identical environment every invocation shares, invokes
//! `env!("CARGO")` (never a PATH lookup), and runs the child under the shared
//! bounded runner, which drains both pipes concurrently while polling for
//! exit — a voluminous build (a cold `--message-format=json` run reports
//! every fresh unit) would otherwise fill the pipe buffer and deadlock the
//! child on a blocking write, making it look hung until the wall-clock bound
//! fires. The bound itself is not reimplemented here: the shared runner caps
//! its post-kill drain, and a copy that did not would hang past its own
//! deadline exactly when a killed `cargo` left a `rustc` holding the pipe.
//!
//! Environment hygiene is the single most likely spurious-red cause: a
//! rebuild triggered by an environment difference between run 1 and run 2 of
//! the rebuild experiment would fail the test for the wrong reason. The child
//! properties are therefore inherited wholesale from the parent and only
//! surgically adjusted — `CARGO_MAKEFLAGS` (a jobserver a nested process
//! cannot open, a classic Windows stall), `CARGO_PKG_*` (they describe a
//! different crate), and `CARGO_LLVM_COV*` (they control the outer coverage
//! driver) are stripped. `CARGO_TARGET_DIR` is preserved when the parent has
//! one and otherwise falls back to the shared workspace `target/`, and
//! `LLVM_PROFILE_FILE` is redirected to a scratch directory so nested coverage
//! never merges into the parent's gated profile.
//!
//! Because an inherited `CARGO_TARGET_DIR` wins, the directory that actually
//! receives the nested artefacts is not always the workspace `target/`.
//! [`child_target_directory`] is the single resolver for it, and the
//! preparation stamp lives inside whatever it returns, so the stamp and the
//! artefacts it vouches for cannot drift apart.

use std::{
    io,
    path::Path,
    process::{Command, Output},
    time::Duration,
};

use serde_json::Value;

/// Per-invocation wall-clock bound for each nested `cargo` call made by a
/// timed assertion. Realistic warm invocations finish in well under 60 s even
/// on a two-core runner; this bound exists to turn a hang into a named,
/// readable failure rather than letting nextest's slow-timeout kill the test
/// with a runner-generated message. It is not a build budget and must not be
/// widened to absorb cold compilation: that work belongs to preparation.
const CHILD_TIMEOUT: Duration = Duration::from_secs(300);

/// Wall-clock bound for one preparation command, re-exported so the
/// preparation module sizes its commands from the shared contract.
pub(crate) use rstest_bdd_harness::nested_cargo::PREPARATION_COMMAND_TIMEOUT;

/// The environment applied to every nested `cargo` invocation, built once so
/// run 1 and run 2 of the rebuild experiment are byte-identical.
pub(crate) struct ChildEnv {
    pub(crate) vars: Vec<(std::ffi::OsString, std::ffi::OsString)>,
    /// The directory the filtered environment sends build artefacts to.
    ///
    /// Resolved once, from the same variable set the child receives, so the
    /// preparation stamp and the nested build can never disagree about where
    /// the artefacts live.
    target_directory: std::path::PathBuf,
}

/// Capture a snapshot of the parent environment, adjusted for the nested
/// build. Every invocation of the child uses this exact snapshot.
pub(crate) fn build_child_env() -> ChildEnv { child_env_from_vars(std::env::vars_os().collect()) }

/// The directory this environment resolves for Cargo's build artefacts.
pub(crate) fn child_target_directory(env: &ChildEnv) -> &Path { &env.target_directory }

/// Filter one environment snapshot into the deterministic nested-Cargo environment.
///
/// Visible to the preparation module's tests so they can pin the resolved
/// target directory without mutating this process's environment.
pub(crate) fn child_env_from_vars(
    mut vars: Vec<(std::ffi::OsString, std::ffi::OsString)>,
) -> ChildEnv {
    vars.retain(|(key, _)| {
        let key = key.to_string_lossy();
        key != "CARGO_MAKEFLAGS"
            && !key.starts_with("CARGO_PKG_")
            && !key.starts_with("CARGO_LLVM_COV")
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
    if vars
        .iter()
        .any(|(key, _)| key.to_string_lossy() == "LLVM_PROFILE_FILE")
    {
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

    // Resolve a relative inherited value once. Cargo reads
    // `CARGO_TARGET_DIR` relative to the working directory of the command
    // that consults it, and every nested build here runs in the scratch
    // fixture directory rather than in the workspace, so the raw value would
    // name one place for this process and another for the child. Absolutising
    // it before it is stored — and therefore before it is handed to the child
    // — keeps the stamp and the artefacts in the same directory.
    vars.sort_by(|a, b| a.0.cmp(&b.0));
    let target_directory = vars
        .iter()
        .find(|(key, _)| key.to_string_lossy() == "CARGO_TARGET_DIR")
        .map_or_else(super::fixtures::shared_target_dir, |(_, value)| {
            rstest_bdd_harness::nested_cargo::absolute_target_directory(value)
        });
    for (key, value) in &mut vars {
        if key.to_string_lossy() == "CARGO_TARGET_DIR" {
            *value = target_directory.clone().into_os_string();
        }
    }
    ChildEnv {
        vars,
        target_directory,
    }
}

/// The preparation policy's view of this harness's child environment.
///
/// The harness keeps its own [`ChildEnv`] so the two runs of each experiment
/// stay byte-identical, including the coverage redirection that the shared
/// helper performs only for [`ProfileDestination::ChildScratch`]. The view
/// exists so the fingerprints, and therefore the stamps, of the preparation
/// test and of the timed assertions are computed from the same variables.
///
/// The fallback target directory is the one the child environment already
/// resolved, and the child environment always carries `CARGO_TARGET_DIR` by
/// this point, so the fingerprint records where the artefacts really go rather
/// than a workspace default the child never uses.
pub(crate) fn preparation_view(env: &ChildEnv) -> rstest_bdd_harness::nested_cargo::NestedCargoEnv {
    rstest_bdd_harness::nested_cargo::env_from_vars(
        env.vars.clone(),
        Some(child_target_directory(env)),
        rstest_bdd_harness::nested_cargo::ProfileDestination::ChildScratch,
    )
}

/// Build a `cargo` invocation against `.env_clear()` plus the shared child
/// environment, so every invocation is byte-identical.
pub(crate) fn cargo_command(env: &ChildEnv, cwd: &Path) -> Command {
    let mut cmd = Command::new(env!("CARGO"));
    // CI forces `CARGO_TERM_COLOR=always`, but this harness matches plain-text
    // Cargo status lines, so each nested command must disable colour itself.
    cmd.args(["--color", "never"]);
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
/// exceeds [`PREPARATION_COMMAND_TIMEOUT`], and return its output.
///
/// Used only by fixture preparation, which compiles the dependency graph the
/// timed assertions used to compile inside their own 300-second bound.
pub(crate) fn run_preparation_bounded(cmd: &mut Command) -> io::Result<Output> {
    run_bounded_with_timeout(cmd, PREPARATION_COMMAND_TIMEOUT)
}

/// Run a child process to completion, killing it with a clear message if it
/// exceeds `CHILD_TIMEOUT`, and return its output.
pub(crate) fn run_bounded(cmd: &mut Command) -> io::Result<Output> {
    run_bounded_with_timeout(cmd, CHILD_TIMEOUT)
}

/// [`run_bounded`] with an explicit wall-clock bound.
///
/// Delegates to the shared runner so the bound has exactly one implementation:
/// a local copy here would keep the killed-child drain-hang bug that the
/// shared one no longer has, and the timed assertions would be the tests that
/// discovered it.
fn run_bounded_with_timeout(cmd: &mut Command, timeout: Duration) -> io::Result<Output> {
    rstest_bdd_harness::nested_cargo::run_bounded_with_timeout(cmd, timeout)
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

#[cfg(test)]
mod tests {
    //! Environment-isolation regression tests for nested Cargo invocations.

    use super::*;

    #[test]
    fn child_environment_strips_llvm_cov_control_variables() {
        let env = child_env_from_vars(vec![
            ("CARGO_LLVM_COV".into(), "1".into()),
            (
                "CARGO_LLVM_COV_TARGET_DIR".into(),
                "/temporary/coverage-target".into(),
            ),
            (
                "CARGO_LLVM_COV_BUILD_DIR".into(),
                "/temporary/coverage-build".into(),
            ),
            ("RSTEST_BDD_NESTED_CARGO_SENTINEL".into(), "kept".into()),
        ]);
        assert!(
            env.vars
                .iter()
                .all(|(key, _)| !key.to_string_lossy().starts_with("CARGO_LLVM_COV"))
        );
        assert!(
            env.vars.iter().any(|(key, value)| {
                key == "RSTEST_BDD_NESTED_CARGO_SENTINEL" && value == "kept"
            })
        );
    }
}
