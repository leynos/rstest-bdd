//! Preparation for the nested build trees this trybuild binary drives.
//!
//! `step_macros_compile` shells out twice. `trybuild` generates a crate under
//! `<target>/tests/trybuild/` and builds it, and the UI-lint cases run `cargo
//! clippy` against `tests/ui_lints` under its own target directory. On a clean
//! checkout, or after the target cache is evicted, both compile a full
//! dependency graph inside that test's own deadline — which is how
//! `step_macros_compile` reached its former twenty-minute limit.
//!
//! Those cold graphs are built here instead, by the `nested_fixture_preparation`
//! test that `.config/nextest.toml` schedules first with a budget of its own.
//! Each family stamps the directory its own artefacts live in:
//!
//! * the trybuild family stamps `<target>/tests/trybuild`, which holds the generated crate and
//!   every dependency unit it links against, and which `staging::stage_trybuild_support_files`
//!   never removes;
//! * the UI-lint family stamps its dedicated `target/tests/ui_lints_clippy` directory, which
//!   nothing else touches.
//!
//! Because a stamp lives inside the tree it describes, evicting that tree also
//! evicts the stamp, so the next run prepares again.
//!
//! Both preparations reuse the code path the timed test uses — `trybuild` with
//! one compile-pass fixture, and the same `cargo clippy` command line the lint
//! cases build. A preparation that took a shortcut would warm a different
//! graph than the one the assertions need.

use std::{env, io, path::PathBuf, process::Command};

use camino::{Utf8Path, Utf8PathBuf};
use rstest_bdd_harness::nested_cargo;

use super::{macros_fixture, staging};

/// Preparation family for the trybuild build tree.
const TRYBUILD_FAMILY: &str = "trybuild-macros";

/// Preparation family for the UI-lint Clippy build tree.
const UI_CLIPPY_FAMILY: &str = "trybuild-ui-clippy";

/// Compile-pass fixture that gives the generated trybuild crate a bin target.
///
/// The crate's dependency graph comes from this crate's manifest, not from
/// which fixtures are queued, so one existing warning-free fixture warms
/// everything the timed run needs and leaves every fixture under test, every
/// `.stderr` snapshot and `tracking.feature` untouched.
const TRYBUILD_PROBE: &str = "step_macros.rs";

/// Lint-clean bin that gives the UI-lint crate a target.
const UI_CLIPPY_PROBE: &str = "step_return_dispatch_lint_clean";

/// The workspace root, two levels above this crate's manifest directory.
fn workspace_root() -> Utf8PathBuf {
    Utf8Path::new(env!("CARGO_MANIFEST_DIR"))
        .parent()
        .and_then(Utf8Path::parent)
        .map_or_else(
            || panic!("workspace root must be two levels above the manifest directory"),
            Utf8Path::to_owned,
        )
}

/// The Cargo target directory this binary's nested builds use.
///
/// Resolved through [`staging::trybuild_target_directory`] so the preparation
/// and the timed test describe the same root, including a coverage-specific
/// `--target-dir` selected by `cargo llvm-cov`.
pub(crate) fn target_directory() -> Utf8PathBuf {
    staging::trybuild_target_directory(&workspace_root())
}

/// The UI-lint crate's own target directory.
///
/// [`ui_clippy_command`] keeps lint runs out of the workspace target so they
/// cannot pollute the shared artefacts, so the UI-lint family stamps here.
fn ui_clippy_target_directory() -> Utf8PathBuf {
    Utf8Path::new(env!("CARGO_MANIFEST_DIR")).join("target/tests/ui_lints_clippy")
}

/// The inputs whose change invalidates a prepared build.
///
/// Both nested crates depend on workspace crates by path, so a change to any
/// crate source or manifest can alter what they compile, and a lockfile change
/// can alter the versions they resolve. `nested_cargo::input_hash` skips every
/// `target` directory, so build output never invalidates its own stamp.
fn input_roots() -> Vec<PathBuf> {
    let workspace = workspace_root();
    ["Cargo.toml", "Cargo.lock", "crates"]
        .into_iter()
        .map(|entry| workspace.join(entry).into_std_path_buf())
        .collect()
}

/// The variant label that records what the trybuild preparation resolved.
///
/// `trybuild` reads the running test binary's feature list and enables only
/// those features on the generated crate, so preparing under a different
/// selection warms a different graph. The label names the selection this
/// binary was built with.
fn trybuild_variant() -> String {
    let mut enabled = Vec::new();
    for (is_enabled, name) in [
        (cfg!(feature = "diagnostics"), "diagnostics"),
        (cfg!(feature = "mutable_world_macro"), "mutable_world_macro"),
        (cfg!(feature = "test-support"), "test-support"),
    ] {
        if is_enabled {
            enabled.push(name);
        }
    }
    if enabled.is_empty() {
        "no-features".to_owned()
    } else {
        enabled.join(",")
    }
}

/// Describe the trybuild preparation family.
fn trybuild_preparation(target_directory: &Utf8Path) -> io::Result<nested_cargo::Preparation> {
    let directory = target_directory.join("tests/trybuild");
    Ok(nested_cargo::Preparation::for_fixture(
        TRYBUILD_FAMILY,
        directory.into_std_path_buf(),
        trybuild_variant(),
        nested_cargo::input_hash(&input_roots())?,
    ))
}

/// Describe the UI-lint preparation family.
fn ui_clippy_preparation() -> io::Result<nested_cargo::Preparation> {
    Ok(nested_cargo::Preparation::for_fixture(
        UI_CLIPPY_FAMILY,
        ui_clippy_target_directory().into_std_path_buf(),
        format!("clippy:{}", env!("CARGO_PKG_VERSION")),
        nested_cargo::input_hash(&input_roots())?,
    ))
}

/// Build the `cargo clippy` invocation the UI-lint cases use.
///
/// Shared by the timed cases and the preparation so the two cannot drift: the
/// timed call appends `--` and its deny arguments, the preparation adds none.
pub(crate) fn ui_clippy_command(bin: &str) -> Command {
    let manifest_dir = Utf8Path::new(env!("CARGO_MANIFEST_DIR"));
    let manifest_path = manifest_dir.join("tests/ui_lints/Cargo.toml");
    let mut command = Command::new("cargo");
    command
        .current_dir(manifest_dir.as_std_path())
        .env("CARGO_TARGET_DIR", ui_clippy_target_directory().as_str())
        .arg("clippy")
        .arg("--locked")
        .arg("--manifest-path")
        .arg(manifest_path.as_str())
        .arg("--bin")
        .arg(bin);
    command
}

/// Prepare both families unconditionally, for the named preparation test.
///
/// # Errors
///
/// Returns the nested build's error when either family fails to prepare, and
/// the stamp I/O error when one cannot be written.
pub(crate) fn prepare_all(target_directory: &Utf8Path) -> io::Result<()> {
    let trybuild = trybuild_preparation(target_directory)?;
    nested_cargo::prepare_now(&trybuild, prepare_trybuild)?;
    let ui_clippy = ui_clippy_preparation()?;
    nested_cargo::prepare_now(&ui_clippy, prepare_ui_clippy)
}

/// Gate the timed run on both families having been prepared.
///
/// # Errors
///
/// Returns a [`io::ErrorKind::NotFound`] error naming `make
/// prepare-nested-fixtures` when a stamp is missing or stale under nextest, and
/// the nested build's error when lazy preparation under `cargo test` fails.
pub(crate) fn ensure_prepared(target_directory: &Utf8Path) -> io::Result<()> {
    let trybuild = trybuild_preparation(target_directory)?;
    nested_cargo::ensure_prepared(&trybuild, prepare_trybuild)?;
    let ui_clippy = ui_clippy_preparation()?;
    nested_cargo::ensure_prepared(&ui_clippy, prepare_ui_clippy)
}

/// Compile both nested build trees before the timed trybuild assertion.
///
/// Runs the same scoping the timed test does — the same serialized target
/// root, the same `CARGO_TARGET_DIR` value and the same `RUST_BACKTRACE`
/// removal — so the stamps it writes describe the trees the timed test will
/// use. `.config/nextest.toml` runs this first with the preparation budget.
///
/// # Errors
///
/// Returns the nested build's error when either family fails to prepare, and
/// the stamp I/O error when one cannot be written.
#[test]
#[serial_test::serial(trybuild_target_directory)]
fn nested_fixture_preparation() -> io::Result<()> {
    // The same Windows-under-nextest deadlock `step_macros_compile` avoids:
    // nextest wraps test binaries in a Job Object, and the nested Cargo's
    // inherited capture pipe never closes.
    if cfg!(windows) && env::var_os("NEXTEST_RUN_ID").is_some() {
        return Ok(());
    }
    let target_directory = target_directory();
    temp_env::with_var("CARGO_TARGET_DIR", Some(target_directory.as_str()), || {
        temp_env::with_var_unset("RUST_BACKTRACE", || prepare_all(&target_directory))
    })
}

/// Compile the generated trybuild crate and its dependency graph.
///
/// `TestCases` queues fixtures and runs them from `Drop`, so queueing one
/// compile-pass fixture and dropping the value performs exactly the write and
/// dependency-build phase the timed run performs, without asserting on any
/// fixture under test.
///
/// `trybuild` panics on a missing or failing fixture instead of returning an
/// error, so this never yields `Err`. The `io::Result` is the shape
/// `TrybuildTree::prepare_now` and `ensure_prepared` take, so that a
/// preparation with a fallible first step — as the UI-lint one has — shares the
/// same gate.
#[expect(
    clippy::unnecessary_wraps,
    reason = "the shared preparation contract takes a fallible closure"
)]
fn prepare_trybuild() -> io::Result<nested_cargo::PreparationSummary> {
    let probe = macros_fixture(TRYBUILD_PROBE);
    let tests = trybuild::TestCases::new();
    tests.pass(probe.as_std_path());
    drop(tests);
    // `trybuild` streams its nested Cargo's output to this process's own
    // streams, so the unit count cannot be captured here. The preparation log
    // still records the elapsed time, which is the figure the timeout budget is
    // sized from.
    Ok(nested_cargo::PreparationSummary::default())
}

/// Compile the UI-lint crate's dependency graph.
///
/// The probe bin is lint-clean, so this also proves the fixture crate builds.
/// It deliberately asserts nothing about diagnostics: that stays the timed
/// cases' job.
///
/// The child is bounded by [`nested_cargo::PREPARATION_COMMAND_TIMEOUT`] like
/// every other preparation command. `Command::output()` would let a wedged
/// `cargo clippy` — waiting on the package-cache lock, say — hang the run with
/// no report: under nextest it would spend the preparation test's `slow-timeout`
/// and lose its output, and under plain `cargo test` the lazy path has no
/// runner bound at all.
fn prepare_ui_clippy() -> io::Result<nested_cargo::PreparationSummary> {
    let mut command = ui_clippy_command(UI_CLIPPY_PROBE);
    let output = nested_cargo::run_bounded_with_timeout(
        &mut command,
        nested_cargo::PREPARATION_COMMAND_TIMEOUT,
    )?;
    if !output.status.success() {
        return Err(io::Error::other(format!(
            "preparing the UI-lint fixture failed:\nstdout:\n{}\nstderr:\n{}",
            String::from_utf8_lossy(&output.stdout),
            String::from_utf8_lossy(&output.stderr),
        )));
    }
    Ok(nested_cargo::PreparationSummary {
        compiled_units: count_checked_units(&output.stderr),
    })
}

/// Count the units Clippy reported.
///
/// Clippy says `Checking` where `cargo build` says `Compiling`; counting both
/// keeps a mixed log accurate.
fn count_checked_units(stderr: &[u8]) -> usize {
    String::from_utf8_lossy(stderr)
        .lines()
        .filter(|line| {
            let line = line.trim_start();
            line.starts_with("Compiling ") || line.starts_with("Checking ")
        })
        .count()
}
