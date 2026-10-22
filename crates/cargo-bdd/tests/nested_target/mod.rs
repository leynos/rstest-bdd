//! Behavioural coverage for the nested build's target directory.
//!
//! `cli_child_environment` decides which `target/` the CLI fixture's nested
//! `cargo` build writes to. That choice is the difference between sharing the
//! workspace's compiled units and rebuilding the fixture's graph from scratch,
//! so these tests observe the directory the build resolves to rather than
//! asserting the environment map the build was handed: the map is an internal
//! representation, while the directory is the effect the timeout budget and the
//! lockfile-alignment gate depend on.
//!
//! Both tests drive the helper with an explicit variable snapshot. Reading the
//! ambient process environment would let a caller-set `CARGO_TARGET_DIR` (as CI
//! coverage sets one) silently displace the fallback each test exists to check.

use std::{
    env,
    ffi::OsString,
    fs,
    path::{Path, PathBuf},
};

use eyre::Result;
use rstest_bdd_harness::nested_cargo;
use serde::Deserialize;
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

/// The `cargo metadata` fields this module reads.
#[derive(Debug, Deserialize)]
struct MetadataTargets {
    target_directory: PathBuf,
}

/// Ask `cargo` where a build with *snapshot* would place its artefacts.
///
/// `cargo metadata` applies the same `CARGO_TARGET_DIR`-before-fallback rule a
/// build applies, and answers with the directory it would use without
/// compiling anything. That makes it a cheap positive witness: it names a
/// directory, where observing a build's refusal instead would depend on
/// failure text that is not the same everywhere — a nested build handed an
/// unusable target directory reports it on one platform and is not
/// consistently stopped by it on another.
///
/// The command runs through the same [`nested_cargo`] helper and the same
/// environment map the fixture build receives, so what it resolves is the
/// environment decision under test rather than a re-derivation of it.
fn resolved_target_directory(snapshot: Vec<(OsString, OsString)>) -> Result<PathBuf> {
    let env = cli_child_environment(snapshot);
    let mut cmd = nested_cargo::cargo_command(&env, &fixture_dir());
    cmd.args(["metadata", "--format-version", "1", "--no-deps"]);
    let output = cmd.output()?;
    let stderr = String::from_utf8_lossy(&output.stderr);
    if !output.status.success() {
        eyre::bail!(
            "`cargo metadata` failed for the fixture ({}); stderr was: {stderr}",
            output.status
        );
    }
    let metadata: MetadataTargets = serde_json::from_slice(&output.stdout)
        .map_err(|err| eyre::eyre!("cargo metadata emitted unexpected JSON: {err}"))?;
    Ok(metadata.target_directory)
}

/// Whether *left* and *right* name the same directory.
///
/// An exact match is the common case and is checked first, because `cargo`
/// echoes a well-formed value unchanged. Canonicalization is the fallback for
/// the platform renderings of one location: Windows may answer with a
/// short-name or extended-length form of the path the test supplied, and the
/// fixture's own directory can be reached through a symlink. Both sides are
/// canonicalized together, so a path that exists on only one side never
/// compares equal by accident of which lookup succeeded.
fn names_same_directory(left: &Path, right: &Path) -> bool {
    if left == right {
        return true;
    }
    match (fs::canonicalize(left), fs::canonicalize(right)) {
        (Ok(left), Ok(right)) => left == right,
        _ => false,
    }
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
    // The caller's directory is named but never built into, so the nested
    // `cargo` reaches its answer without compiling and without needing the
    // directory to be usable. The observation is which directory it names: the
    // caller's, not the workspace fallback.
    let scratch = tempfile::tempdir()?;
    let caller_target = scratch.path().join("caller-target");
    // Created, but left empty: `cargo metadata` answers with the directory
    // without building into it, and an existing directory lets both sides of
    // the comparison be canonicalized when a platform renders the same place
    // two ways.
    fs::create_dir(&caller_target)?;

    let mut snapshot = ambient_without_target_dir();
    snapshot.push((
        OsString::from("CARGO_TARGET_DIR"),
        caller_target.as_os_str().to_os_string(),
    ));

    let resolved = resolved_target_directory(snapshot)?;

    assert!(
        names_same_directory(&resolved, &caller_target),
        "a caller-supplied `CARGO_TARGET_DIR` must take precedence over the workspace fallback \
         `{}`; cargo resolved `{}`",
        workspace_target().display(),
        resolved.display()
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

    let snapshot = ambient_without_target_dir();
    let resolved = resolved_target_directory(snapshot.clone())?;
    assert!(
        names_same_directory(&resolved, &workspace_target()),
        "an absent `CARGO_TARGET_DIR` must select the workspace fallback; cargo resolved `{}`",
        resolved.display()
    );

    let output = run_fixture_with(snapshot)?;
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
