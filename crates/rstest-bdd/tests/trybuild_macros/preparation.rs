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

use std::{
    env,
    io,
    path::{Path, PathBuf},
};

use camino::{Utf8Path, Utf8PathBuf};
use rstest_bdd_harness::{macrotest_support::reject_trybuild_filter, nested_cargo};

use super::{macros_fixture, staging, wip};

// The UI-lint family is a module of its own: it shares no constant, no command
// line and no stamped root with the trybuild family below, and splitting it out
// keeps both files under the repository's per-file line budget.
#[path = "preparation/ui_clippy.rs"]
mod ui_clippy;
pub(crate) use ui_clippy::ui_clippy_command;

/// Preparation family for the trybuild build tree.
const TRYBUILD_FAMILY: &str = "trybuild-macros";

/// Compile-pass fixture that gives the generated trybuild crate a bin target.
///
/// The crate's dependency graph comes from this crate's manifest, not from
/// which fixtures are queued, so one existing warning-free fixture warms
/// everything the timed run needs and leaves every fixture under test, every
/// `.stderr` snapshot and `tracking.feature` untouched.
const TRYBUILD_PROBE: &str = "step_macros.rs";

/// Compile-fail fixture that warms the check-mode dependency units.
///
/// `trybuild` runs `cargo build` for the generated crate when any queued
/// fixture is a compile-pass case and `cargo check` otherwise, so the timed
/// `compile_fail_with_normalized_output` cases run in check mode against the
/// same target directory. Cargo keeps check-mode and build-mode artefacts
/// apart — measured on this host: a `cargo check` following a warm `cargo
/// build` re-checks every dependency, including registry crates — so warming
/// only the build graph would leave the first timed compile-fail case
/// compiling the graph in check mode inside its own deadline.
///
/// The probe must be a fixture whose snapshot names no directory. `trybuild`
/// compares diagnostics verbatim, and a preparation run is not the timed run:
/// it queues this one fixture and nothing else, so nothing calls
/// `staging::stage_target_root_snapshots` to rewrite the expected text for the
/// coverage target root. `scenario_missing_file`, the first choice, names the
/// target directory in its diagnostic, so as a probe it reported a mismatch —
/// the checked-in `$WORKSPACE/target/...` against the actual
/// `$WORKSPACE/target/llvm-cov-target/...` — on both coverage lanes. This
/// fixture fails while parsing the macro arguments, so it opens no file and its
/// diagnostic is the same under any target root.
///
/// The probe's own diagnostics are never asserted on: `trybuild` writes them
/// to `wip/` and the file is removed below, because a stale `wip` entry makes
/// `trybuild` report a newly created `.stderr` as a failure.
const TRYBUILD_CHECK_PROBE: &str = "scenario_missing_path.rs";

/// The workspace root, two levels above this crate's manifest directory.
///
/// Resolved once here rather than at each comparison.
/// [`nested_cargo::cargo_config_inputs`] canonicalizes the root it is given and
/// reports the workspace configuration through that resolved form, so a caller
/// holding the lexical spelling beside a reported entry would be comparing two
/// spellings of a single directory. On Windows the difference is visible:
/// resolution returns the `\\?\` verbatim prefix, which no plain-spelled prefix
/// — and no plainly spelled path, for `contains` — can match. Resolving at the
/// definition means every caller compares in one form and none has to remember
/// to.
///
/// Kept a [`Utf8PathBuf`] because [`staging::trybuild_target_directory`] and
/// [`input_roots`] both take or produce camino paths; the resolution happens
/// through `std` and is converted back.
fn workspace_root() -> Utf8PathBuf {
    let lexical = Utf8Path::new(env!("CARGO_MANIFEST_DIR"))
        .parent()
        .and_then(Utf8Path::parent)
        .map_or_else(
            || panic!("workspace root must be two levels above the manifest directory"),
            Utf8Path::to_owned,
        );
    // A root that cannot be resolved is left lexical: the manifests this root
    // locates report that absence themselves, with the path they tried.
    let resolved = std::fs::canonicalize(lexical.as_std_path())
        .ok()
        .and_then(|path| Utf8PathBuf::from_path_buf(path).ok());
    resolved.unwrap_or(lexical)
}

/// The Cargo target directory this binary's nested builds use.
///
/// Resolved through [`staging::trybuild_target_directory`] so the preparation
/// and the timed test describe the same root, including a coverage-specific
/// `--target-dir` selected by `cargo llvm-cov`.
pub(crate) fn target_directory() -> Utf8PathBuf {
    staging::trybuild_target_directory(&workspace_root())
}

/// The inputs whose change invalidates a prepared build.
///
/// Both nested crates depend on workspace crates by path, so a change to any
/// crate source or manifest can alter what they compile, and a lockfile change
/// can alter the versions they resolve. `nested_cargo::input_hash` skips every
/// `target` directory, so build output never invalidates its own stamp.
///
/// The workspace's Cargo configuration belongs here too: both nested builds run
/// beneath the workspace, so Cargo applies it, and changing the development
/// profile options or the Linux linker flags rebuilds dependency artefacts
/// without touching a fixture source.
///
/// # Errors
///
/// Returns the error when a workspace Cargo configuration path exists but
/// cannot be inspected, so an unreadable configuration refuses the
/// preparation instead of silently leaving it out of the input hash.
fn input_roots() -> io::Result<Vec<PathBuf>> {
    let workspace = workspace_root();
    let mut roots: Vec<PathBuf> = ["Cargo.toml", "Cargo.lock", "crates"]
        .into_iter()
        .map(|entry| workspace.join(entry).into_std_path_buf())
        .collect();
    roots.extend(nested_cargo::cargo_config_inputs(workspace.as_std_path())?);
    // `input_roots` serves both nested builds, and they do not run from the
    // same directory: `ui_clippy_command` sets `current_dir` to
    // `CARGO_MANIFEST_DIR`, which is `crates/rstest-bdd`, so Cargo's upward walk
    // reads a different set of `.cargo` directories than the trybuild build
    // does. Collecting only from the workspace root left anything found between
    // that root and the manifest directory out of the hash, and a relative
    // `CARGO_HOME` is resolved against the command's directory as well, so the
    // same value names a different tree for the two builds.
    roots.extend(nested_cargo::cargo_config_inputs(Path::new(env!(
        "CARGO_MANIFEST_DIR"
    )))?);
    roots.sort();
    roots.dedup();
    Ok(roots)
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
    // `trybuild` runs its nested Cargo inside the generated crate, which is a
    // child of the stamped root, so that is the directory a relative `RUSTC`,
    // `CARGO_HOME` or `CARGO_BUILD_*` override resolves against. The crate is
    // named after this package, matching `TARGET_SUBDIR` in the server suite.
    let generated = directory.join(env!("CARGO_PKG_NAME"));
    nested_cargo::Preparation::for_fixture(
        nested_cargo::Sources::new(
            TRYBUILD_FAMILY,
            trybuild_variant(),
            nested_cargo::input_hash(&input_roots()?)?,
        ),
        nested_cargo::Directories::new(
            directory.into_std_path_buf(),
            generated.into_std_path_buf(),
        ),
    )
}

/// Prepare both families unconditionally, for the named preparation test.
///
/// "Unconditionally" is about the stamp: [`nested_cargo::prepare_now`] rebuilds
/// even when a current stamp is present, which is what
/// `make prepare-nested-fixtures` needs. It still returns at a stamp the gate
/// accepts, so both the trybuild-filter refusal and the rebuild are reached
/// only when the family is actually prepared.
///
/// # Errors
///
/// Returns a [`io::ErrorKind::InvalidInput`] error when a family has to prepare
/// and the run carries a `trybuild=` fixture filter, the nested build's error
/// when either family fails to prepare, and the stamp I/O error when one cannot
/// be written.
pub(crate) fn prepare_all(target_directory: &Utf8Path) -> io::Result<()> {
    let trybuild = trybuild_preparation(target_directory)?;
    nested_cargo::prepare_now(&trybuild, prepare_trybuild)?;
    let ui_clippy = ui_clippy::ui_clippy_preparation()?;
    nested_cargo::prepare_now(&ui_clippy, ui_clippy::prepare_ui_clippy)
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
    let ui_clippy = ui_clippy::ui_clippy_preparation()?;
    nested_cargo::ensure_prepared(&ui_clippy, ui_clippy::prepare_ui_clippy)
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
/// error, so the compile steps themselves never yield `Err`. The `io::Result`
/// is both the shape `TrybuildTree::prepare_now` and `ensure_prepared` take and
/// a real one now: the compile-fail probe leaves its diagnostics in `wip/`,
/// and removing them is a filesystem step that can fail, so this propagates
/// that error rather than panicking on it.
///
/// It is also where the `trybuild=` fixture filter is refused. `trybuild`
/// retains only the queued fixtures whose path contains the filter, so a
/// filtered run would queue both probes, drop the ones that do not match, and
/// still leave this closure writing the complete stamp for the family. The
/// timed case would then be admitted into a tree missing whatever mode the
/// dropped probe was meant to warm — the cold-build failure the preparation
/// exists to prevent. Refusing here rather than at the entry points keeps a
/// filtered assertion working over an already-prepared tree, which never
/// reaches this closure because the stamp is accepted first.
fn prepare_trybuild() -> io::Result<nested_cargo::PreparationSummary> {
    reject_trybuild_filter()?;
    // Build mode first, then check mode. `trybuild` decides between `cargo
    // build` and `cargo check` for the whole generated crate from whether any
    // queued fixture is a compile-pass case, so these cannot be combined: each
    // queueing run has to be homogeneous. Order matters because the check run
    // also compiles the build graph's build scripts and proc macros as host
    // units, and doing it second leaves them warm for it.
    let pass_probe = macros_fixture(TRYBUILD_PROBE);
    let tests = trybuild::TestCases::new();
    tests.pass(pass_probe.as_std_path());
    drop(tests);

    let fail_probe = macros_fixture(TRYBUILD_CHECK_PROBE);
    let tests = trybuild::TestCases::new();
    tests.compile_fail(fail_probe.as_std_path());
    drop(tests);
    // The compile-fail run leaves the diagnostics it produced in `wip/`, and
    // `trybuild` reports a fixture as a failure when it finds one there: the
    // timed run would then fail on a `.stderr` this preparation created rather
    // than on the fixture it is asserting. The timed suite removes the same
    // file before each of its own compile-fail cases for the same reason.
    wip::remove_stale_wip_stderr(&fail_probe)?;
    // `trybuild` streams its nested Cargo's output to this process's own
    // streams, so the unit count cannot be captured here. The preparation log
    // still records the elapsed time, which is the figure the timeout budget is
    // sized from.
    Ok(nested_cargo::PreparationSummary::default())
}

#[cfg(test)]
#[path = "preparation/tests.rs"]
mod tests;
