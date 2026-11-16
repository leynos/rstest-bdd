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

use super::{macros_fixture, staging, wip};

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
    nested_cargo::Preparation::for_fixture(
        TRYBUILD_FAMILY,
        directory.into_std_path_buf(),
        trybuild_variant(),
        nested_cargo::input_hash(&input_roots()?)?,
    )
}

/// Describe the UI-lint preparation family.
fn ui_clippy_preparation() -> io::Result<nested_cargo::Preparation> {
    nested_cargo::Preparation::for_fixture(
        UI_CLIPPY_FAMILY,
        ui_clippy_target_directory().into_std_path_buf(),
        format!("clippy:{}", env!("CARGO_PKG_VERSION")),
        nested_cargo::input_hash(&input_roots()?)?,
    )
}

/// Build the `cargo clippy` invocation the UI-lint cases use.
///
/// Shared by the timed cases and the preparation so the two cannot drift: the
/// timed call appends `--` and its deny arguments, the preparation adds none.
///
/// The environment is applied here rather than left to the caller, so a
/// command built for one mode cannot run with a different one's variables. It
/// is resolved the same way [`nested_cargo::Preparation::for_fixture`] resolves
/// it for this family — [`nested_cargo::env_for_directory`] over the stamped
/// directory — because that is the environment the preparation warmed the
/// graph under. Handing the child this process's inherited variables instead
/// would leave `CARGO_MAKEFLAGS` and `CARGO_PKG_*` set: the former names a
/// jobserver the child cannot open and stalls on Windows, and the latter
/// describes a different crate. The target directory is therefore set *before*
/// `apply_to`, which would otherwise clear it.
pub(crate) fn ui_clippy_command(bin: &str) -> io::Result<Command> {
    let manifest_dir = Utf8Path::new(env!("CARGO_MANIFEST_DIR"));
    let manifest_path = manifest_dir.join("tests/ui_lints/Cargo.toml");
    let mut command = Command::new("cargo");
    let environment = nested_cargo::env_for_directory(
        ui_clippy_target_directory().as_std_path(),
        nested_cargo::ProfileDestination::Caller,
    )?;
    // `apply_to` clears the environment first, so it must run before the
    // target directory is set: anything set earlier would be discarded.
    environment.apply_to(&mut command);
    command
        .current_dir(manifest_dir.as_std_path())
        .env("CARGO_TARGET_DIR", ui_clippy_target_directory().as_str())
        .arg("clippy")
        .arg("--locked")
        .arg("--manifest-path")
        .arg(manifest_path.as_str())
        .arg("--bin")
        .arg(bin);
    Ok(command)
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
/// error, so the compile steps themselves never yield `Err`. The `io::Result`
/// is both the shape `TrybuildTree::prepare_now` and `ensure_prepared` take and
/// a real one now: the compile-fail probe leaves its diagnostics in `wip/`,
/// and removing them is a filesystem step that can fail, so this propagates
/// that error rather than panicking on it.
fn prepare_trybuild() -> io::Result<nested_cargo::PreparationSummary> {
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
    let mut command = ui_clippy_command(UI_CLIPPY_PROBE)?;
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

#[cfg(test)]
mod tests {
    //! Regression coverage for the inputs that invalidate a prepared tree.

    use rstest_bdd_harness::nested_cargo;

    use super::input_roots;

    #[test]
    fn the_workspace_cargo_configuration_is_an_input() {
        // Both nested builds run beneath the workspace, so Cargo applies the
        // committed `.cargo/config.toml`: its profile options and Linux linker
        // flags change dependency artefacts without changing a fixture source.
        // A stamp that ignored it would survive a configuration-only edit and
        // the timed assertion would compile the graph instead.
        //
        // Removing the root from the same list, rather than hashing a different
        // tree, isolates the configuration's contribution: every other input is
        // identical between the two hashes.
        let roots = input_roots().expect("input roots");
        let workspace_config = super::workspace_root()
            .join(".cargo/config.toml")
            .into_std_path_buf();
        assert!(
            roots.contains(&workspace_config),
            "expected {} among the preparation inputs: {roots:?}",
            workspace_config.display()
        );

        let with = nested_cargo::input_hash(&roots).expect("hash with configuration");
        let without: Vec<std::path::PathBuf> = roots
            .into_iter()
            .filter(|root| root != &workspace_config)
            .collect();
        let without = nested_cargo::input_hash(&without).expect("hash without configuration");
        assert_ne!(
            with, without,
            "the workspace Cargo configuration must change the input hash"
        );
    }
}
