//! Fixture locations, manifest reading, and the nested-Cargo boundary.
//!
//! Everything the parent module and the preparation submodule share lives
//! here: where the fixtures are, how a manifest's declared dependencies are
//! read, and the command shape both the preparation and the timed run use, so
//! the two cannot warm different graphs.

use std::{
    env,
    fmt,
    io,
    path::{Path, PathBuf},
    process::Command,
    time::Duration,
};

use cap_std::{ambient_authority, fs::Dir};
use rstest_bdd_harness::nested_cargo::{self, NestedCargoEnv, ProfileDestination};
use toml::Value;

/// The name of one fixture crate beneath `tests/fixtures/`.
///
/// A newtype rather than a plain `&str`: nearly every function here takes a
/// fixture name beside another string — a test-source file name, a dependency
/// name — and a bare `&str` would let a caller supply one where the other was
/// meant without a compile error.
#[derive(Clone, Copy, Debug)]
pub(crate) struct Fixture(&'static str);

impl Fixture {
    /// The fixture's directory name beneath `tests/fixtures/`.
    pub(crate) fn as_str(self) -> &'static str { self.0 }
}

impl fmt::Display for Fixture {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result { formatter.write_str(self.0) }
}

/// Fixture directory names under `tests/fixtures/`.
pub(crate) const PRELUDE_ONLY: Fixture = Fixture("prelude_only");
pub(crate) const MACROS_DIRECT: Fixture = Fixture("macros_direct");

/// The crate the `rstest-bdd` facade re-exports, and which a consumer writing
/// through the prelude must not name.
pub(crate) const MACROS_CRATE: &str = "rstest-bdd-macros";

/// The dependency tables a Cargo manifest can declare a crate in.
const DEPENDENCY_TABLES: [&str; 3] = ["dependencies", "dev-dependencies", "build-dependencies"];

/// Every fixture this binary drives, in the order the preparation builds them.
pub(crate) const FIXTURES: [Fixture; 2] = [PRELUDE_ONLY, MACROS_DIRECT];

/// Wall-clock bound for one fixture run.
///
/// A warm run compiles nothing and finishes in seconds. The bound exists to
/// turn a hang into a named failure that carries the captured output, rather
/// than letting nextest's slow-timeout kill the test with a runner-generated
/// message; cold work belongs to preparation, not here.
pub(crate) const RUN_TIMEOUT: Duration = Duration::from_secs(300);

/// The source directory of one fixture crate.
pub(crate) fn fixture_directory(fixture: Fixture) -> PathBuf {
    fixtures_root().join(fixture.as_str())
}

/// The directory holding both consumer fixtures.
///
/// The preparation family covers both fixtures but can record one working
/// directory, so it names their common parent rather than either fixture.
pub(crate) fn fixtures_root() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/fixtures")
}

/// The workspace's shared build-output directory.
///
/// Used only when the parent process names no `CARGO_TARGET_DIR` of its own;
/// an inherited value wins, which is what keeps a coverage run's target
/// directory warm.
pub(crate) fn shared_target_directory() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).join("../../target")
}

/// Read a fixture's manifest as parsed TOML.
pub(crate) fn read_manifest(fixture: Fixture) -> io::Result<Value> {
    let directory = open_fixture(fixture)?;
    let text = directory.read_to_string("Cargo.toml")?;
    toml::from_str(&text).map_err(|error| {
        io::Error::new(
            io::ErrorKind::InvalidData,
            format!(
                "{} is not valid TOML: {error}",
                fixture_directory(fixture).join("Cargo.toml").display()
            ),
        )
    })
}

/// Open one fixture crate's directory with ambient authority.
///
/// Every read below goes through a `cap_std` directory handle rather than
/// `std::fs`, so a path component the fixture supplies cannot escape the
/// directory the test opened.
fn open_fixture(fixture: Fixture) -> io::Result<Dir> {
    Dir::open_ambient_dir(fixture_directory(fixture), ambient_authority())
}

/// Every crate name a manifest declares, across all dependency tables.
///
/// Sorted and de-duplicated so an assertion over the result reads as a set
/// comparison and its failure message is stable.
pub(crate) fn declared_dependencies(manifest: &Value) -> Vec<&str> {
    let mut names: Vec<&str> = DEPENDENCY_TABLES
        .iter()
        .filter_map(|table| manifest.get(*table)?.as_table())
        .flat_map(|dependencies| dependencies.keys().map(String::as_str))
        .collect();
    names.sort_unstable();
    names.dedup();
    names
}

/// Read one of a fixture's integration-test sources.
pub(crate) fn read_test_source(fixture: Fixture, file: &str) -> io::Result<String> {
    open_fixture(fixture)?.read_to_string(Path::new("tests").join(file))
}

/// Read the workspace root manifest as parsed TOML.
pub(crate) fn read_workspace_manifest() -> io::Result<Value> {
    let root = Path::new(env!("CARGO_MANIFEST_DIR"))
        .ancestors()
        .nth(2)
        .ok_or_else(|| io::Error::other("the crate manifest is two levels under the workspace"))?;
    let workspace = Dir::open_ambient_dir(root, ambient_authority())?;
    let text = workspace.read_to_string("Cargo.toml")?;
    toml::from_str(&text).map_err(|error| {
        io::Error::new(
            io::ErrorKind::InvalidData,
            format!("the workspace Cargo.toml is not valid TOML: {error}"),
        )
    })
}

/// The version requirement one dependency table states for `name`.
///
/// A dependency written as a bare string carries its requirement in the
/// string; a table carries it under `version`.
pub(crate) fn dependency_requirement(manifest: &Value, table: &str, name: &str) -> Option<String> {
    requirement(manifest.get(table)?.get(name)?)
}

/// The version requirement the workspace states for `name`.
pub(crate) fn workspace_requirement(manifest: &Value, name: &str) -> Option<String> {
    requirement(manifest.get("workspace")?.get("dependencies")?.get(name)?)
}

/// Return the requirement a single dependency specification states.
fn requirement(spec: &Value) -> Option<String> {
    match spec {
        Value::String(version) => Some(version.clone()),
        Value::Table(entry) => entry.get("version")?.as_str().map(str::to_owned),
        _ => None,
    }
}

/// The filtered environment every fixture build and run inherits.
///
/// The nested build's own coverage is incidental to the run that spawned it,
/// so it is redirected out of the caller's gated profile rather than merged
/// into it.
pub(crate) fn child_environment() -> io::Result<NestedCargoEnv> {
    nested_cargo::build_child_env(&shared_target_directory(), ProfileDestination::ChildScratch)
}

/// Build the `cargo` invocation for one fixture, with `extra` arguments
/// appended to the shared prefix.
pub(crate) fn fixture_command(
    environment: &NestedCargoEnv,
    fixture: Fixture,
    extra: &[&str],
) -> Command {
    let mut command = nested_cargo::cargo_command(environment, &fixture_directory(fixture));
    command.args(["test", "--locked", "--offline"]).args(extra);
    command
}
