//! Behavioural coverage for the nested build's target directory.
//!
//! `cli_child_environment` decides which `target/` the CLI fixture's nested
//! `cargo` build writes to. That choice is the difference between sharing the
//! workspace's compiled units and rebuilding the fixture's graph from scratch,
//! so these tests observe where a real nested build lands rather than asserting
//! the environment map the build was handed: the map is an internal
//! representation, while the directory is the effect the timeout budget and the
//! lockfile-alignment gate depend on.
//!
//! Both tests drive the helper with an explicit variable snapshot. Reading the
//! ambient process environment would let a caller-set `CARGO_TARGET_DIR` (as CI
//! coverage sets one) silently displace the fallback each test exists to check.

use std::{env, ffi::OsString, fs, path::Path};

use eyre::Result;
use rstest_bdd_harness::nested_cargo;
use serial_test::serial;

use super::{
    cli_child_environment,
    fixture_dir,
    locate_or_build_cargo_bdd_command,
    workspace_root,
};

/// Run `cargo-bdd steps` for the fixture with *snapshot* as the starting
/// environment.
///
/// The snapshot is handed to `cli_child_environment`, so the helper under test
/// is the only thing deciding the child's variable set; the ambient environment
/// is never consulted.
///
/// *snapshot* replaces only the target-directory decision. The caller supplies
/// the ambient variable pairs — `PATH` above all, which the filtered child needs
/// to find `cargo` — and this function strips `CARGO_TARGET_DIR` from them, so
/// the helper's own precedence rule is what the test observes.
fn run_fixture_with(snapshot: Vec<(OsString, OsString)>) -> Result<std::process::Output> {
    let env = cli_child_environment(snapshot);
    let program = locate_or_build_cargo_bdd_command()?;
    let mut cmd =
        nested_cargo::filtered_command(Path::new(program.get_program()), &env, &fixture_dir());
    cmd.args(["steps"]);
    Ok(cmd.output()?)
}

/// The directory the workspace fallback names.
fn workspace_target() -> std::path::PathBuf { workspace_root().join("target") }

/// The ambient variable pairs a real CLI run starts from.
///
/// `cli_child_environment` is applied to the process environment in production,
/// so the tests start from the same pairs. `CARGO_TARGET_DIR` is dropped because
/// a test runner or a coverage job may set one, and leaving it in would decide
/// the test's outcome for it. Without that removal the "absent" case would only
/// be exercised on hosts that happen not to set the variable.
fn ambient_without_target_dir() -> Vec<(OsString, OsString)> {
    env::vars_os()
        .filter(|(key, _)| key != "CARGO_TARGET_DIR")
        .collect()
}

#[test]
#[serial]
fn an_inherited_target_takes_precedence_over_the_workspace_fallback() -> Result<()> {
    // A regular file where the caller's target directory should be cannot hold
    // a build, so the nested `cargo` fails at once and names the path it tried
    // to use. That failure is the observation: it costs no compilation and is
    // reachable only if the child was handed the caller's directory.
    let scratch = tempfile::tempdir()?;
    let caller_target = scratch.path().join("caller-target");
    fs::write(&caller_target, b"not a directory")?;

    let mut snapshot = ambient_without_target_dir();
    snapshot.push((
        OsString::from("CARGO_TARGET_DIR"),
        caller_target.as_os_str().to_os_string(),
    ));

    let output = run_fixture_with(snapshot)?;
    let stderr = String::from_utf8_lossy(&output.stderr);

    assert!(
        stderr.contains(&caller_target.to_string_lossy().into_owned()),
        "the nested build must be told to use the caller's target directory `{}`; stderr was: \
         {stderr}",
        caller_target.display()
    );
    assert!(
        !stderr.contains(&workspace_target().to_string_lossy().into_owned()),
        "a caller-supplied target must take precedence over the workspace fallback; stderr was: \
         {stderr}"
    );
    Ok(())
}

#[test]
#[serial]
fn an_absent_target_falls_back_to_the_workspace_target() -> Result<()> {
    // The fixture's own directory is the witness. Cargo writes
    // `<manifest dir>/target` when nothing redirects it, so that directory's
    // absence after a successful build is what shows the fallback was applied.
    // Running the fixture with the variable genuinely unset does create it, so
    // this discriminates rather than observing an incidental absence.
    let fixture_target = fixture_dir().join("target");
    if fixture_target.exists() {
        fs::remove_dir_all(&fixture_target)?;
    }

    let output = run_fixture_with(ambient_without_target_dir())?;
    let stderr = String::from_utf8_lossy(&output.stderr);

    assert!(
        output.status.success(),
        "the nested build must succeed against the workspace target; stderr was: {stderr}"
    );
    assert!(
        !output.stdout.is_empty(),
        "a successful run must report steps, which requires the nested build to have produced the \
         fixture's test binaries"
    );
    assert!(
        !fixture_target.exists(),
        "the nested build must not write to the fixture's own `{}`; that is the directory an \
         absent `CARGO_TARGET_DIR` selects without the workspace fallback. stderr was: {stderr}",
        fixture_target.display()
    );
    Ok(())
}
