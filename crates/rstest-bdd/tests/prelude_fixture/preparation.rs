//! Cold-graph preparation for the two consumer fixtures.
//!
//! Both fixtures resolve the framework through a lockfile of their own, so
//! their first build after a cache eviction compiles a full dependency graph.
//! That work belongs to the `nested_fixture_preparation` test that
//! `.config/nextest.toml` schedules first with a budget of its own, not to the
//! timed test whose deadline would otherwise pay for it.
//!
//! Both fixtures share one family, one stamp and one target directory: their
//! graphs overlap almost entirely, and a second stamp over the same units would
//! only mean two chances to disagree about whether the tree is current.

use std::{
    env,
    io,
    path::{Path, PathBuf},
};

use rstest_bdd_harness::nested_cargo::{
    self,
    Directories,
    NestedCargoEnv,
    Preparation,
    PreparationSummary,
    Sources,
};

use super::support;

/// Preparation family covering both fixtures.
const FAMILY: &str = "prelude-fixture";

/// The build this family prepares.
///
/// Both fixtures pin the same feature set, so the label describes their
/// manifests rather than this binary's features.
const VARIANT: &str = "prelude,macros-direct,diagnostics,test-support";

/// The workspace root, two levels above this crate's manifest directory.
fn workspace_root() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR"))
        .parent()
        .and_then(Path::parent)
        .map_or_else(
            || panic!("workspace root must be two levels above the manifest directory"),
            Path::to_path_buf,
        )
}

/// The inputs whose change invalidates a prepared build.
///
/// Both fixtures depend on workspace crates by path, so a change to any crate
/// source or manifest can alter what they compile, and a lockfile change can
/// alter the versions they resolve. The fixtures themselves live under
/// `crates/`, so the same walk covers them. `nested_cargo::input_hash` skips
/// every `target` directory, so build output never invalidates its own stamp.
///
/// The workspace's Cargo configuration belongs here too: both nested builds run
/// beneath the workspace, so Cargo applies it, and changing the profile options
/// or the linker flags rebuilds dependency artefacts without touching a fixture
/// source.
///
/// # Errors
///
/// Returns the error when a workspace Cargo configuration path exists but
/// cannot be inspected, so an unreadable configuration refuses the preparation
/// instead of silently leaving it out of the input hash.
fn input_roots() -> io::Result<Vec<PathBuf>> {
    let workspace = workspace_root();
    let mut roots: Vec<PathBuf> = ["Cargo.toml", "Cargo.lock", "crates"]
        .into_iter()
        .map(|entry| workspace.join(entry))
        .collect();
    roots.extend(nested_cargo::cargo_config_inputs(&workspace)?);
    Ok(roots)
}

/// Describe the family: what it builds, where its stamp lives, and the
/// environment its commands run with.
///
/// The stamp lives inside the shared target directory the child builds into,
/// so evicting that tree evicts the stamp and the next run prepares again.
///
/// # Errors
///
/// Returns the error when the child environment cannot be resolved or the
/// input hash cannot be computed.
pub(crate) fn describe() -> io::Result<Preparation> {
    let environment = support::child_environment()?;
    let root = environment
        .target_directory()?
        .ok_or_else(|| {
            io::Error::other("the nested Cargo environment resolves no target directory")
        })?
        .join("tests/prelude-fixture");
    Preparation::new(
        Sources::new(FAMILY, VARIANT, nested_cargo::input_hash(&input_roots()?)?),
        Directories::new(&root, support::fixtures_root()),
        environment,
    )
}

/// Compile both fixtures without running their tests.
///
/// This is the same command line the timed run uses, stopped before execution,
/// so a preparation cannot warm a graph the assertion does not need.
///
/// # Errors
///
/// Returns the nested build's error when either fixture fails to compile,
/// including the captured output, and the timeout error when a command
/// outlives its bound.
pub(crate) fn prepare() -> io::Result<PreparationSummary> {
    let environment = support::child_environment()?;
    let mut compiled_units = 0;
    for fixture in support::FIXTURES {
        compiled_units += build_fixture(&environment, fixture)?;
    }
    Ok(PreparationSummary { compiled_units })
}

/// Compile one fixture, reporting what it cost.
fn build_fixture(environment: &NestedCargoEnv, fixture: support::Fixture) -> io::Result<usize> {
    let mut command = support::fixture_command(environment, fixture, &["--no-run"]);
    let output = nested_cargo::run_bounded_with_timeout(
        &mut command,
        nested_cargo::PREPARATION_COMMAND_TIMEOUT,
    )?;
    if !output.status.success() {
        return Err(io::Error::other(format!(
            "preparing the {fixture} fixture failed:\nstdout:\n{}\nstderr:\n{}",
            String::from_utf8_lossy(&output.stdout),
            String::from_utf8_lossy(&output.stderr),
        )));
    }
    Ok(nested_cargo::count_compiling(&output.stderr))
}

/// Gate a timed run on the family having been prepared.
///
/// Under nextest a missing or stale stamp fails fast, naming the make target
/// that prepares it; under plain `cargo test` the same call prepares lazily
/// behind a process-wide lock. The runner decision is resolved here, as the
/// shared helper requires, so the gate's behaviour is stated by its call site
/// rather than read from the ambient process.
///
/// # Errors
///
/// As [`nested_cargo::ensure_prepared_building`].
pub(crate) fn ensure() -> io::Result<()> {
    nested_cargo::ensure_prepared_building(
        &describe()?,
        nested_cargo::running_under_nextest(),
        prepare,
    )
    .map(|_outcome| ())
}
