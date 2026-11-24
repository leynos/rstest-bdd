//! The UI-lint preparation family: the Clippy tree the lint cases reuse.
//!
//! The timed lint cases run `cargo clippy` against `tests/ui_lints` under a
//! target directory of its own, and the cold dependency graph that command
//! compiles is prepared here instead. The family is a module of its own
//! because it shares nothing with the `trybuild` family beyond the input list:
//! its stamped root, its variant label, its command line and its unit-counting
//! rule are all specific to Clippy.
//!
//! The command is built by one function used both here and by the timed cases,
//! so a preparation cannot warm a graph the assertions then refuse to reuse.

use std::{env, io, path::Path, process::Command};

use camino::{Utf8Path, Utf8PathBuf};
use rstest_bdd_harness::nested_cargo;

use super::input_roots;

/// Preparation family for the UI-lint Clippy build tree.
const UI_CLIPPY_FAMILY: &str = "trybuild-ui-clippy";

/// Lint-clean bin that gives the UI-lint crate a target.
const UI_CLIPPY_PROBE: &str = "step_return_dispatch_lint_clean";

/// The UI-lint crate's own target directory.
///
/// [`ui_clippy_command`] keeps lint runs out of the workspace target so they
/// cannot pollute the shared artefacts, so the UI-lint family stamps here.
fn ui_clippy_target_directory() -> Utf8PathBuf {
    Utf8Path::new(env!("CARGO_MANIFEST_DIR")).join("target/tests/ui_lints_clippy")
}

/// Describe the UI-lint preparation family.
pub(super) fn ui_clippy_preparation() -> io::Result<nested_cargo::Preparation> {
    nested_cargo::Preparation::for_fixture(
        nested_cargo::Sources::new(
            UI_CLIPPY_FAMILY,
            format!("clippy:{}", env!("CARGO_PKG_VERSION")),
            nested_cargo::input_hash(&input_roots()?)?,
        ),
        nested_cargo::Directories::new(
            ui_clippy_target_directory().into_std_path_buf(),
            // `ui_clippy_command` sets `current_dir` to this manifest
            // directory, so it — not the stamped target root — is where Cargo
            // resolves a relative `RUSTC` or `CARGO_HOME` for this family.
            Path::new(env!("CARGO_MANIFEST_DIR")).to_path_buf(),
        ),
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
        // The fixture's lockfile is committed; nothing here may
        // reach a registry while holding the shared package-cache lock.
        .env("CARGO_NET_OFFLINE", "true")
        .arg("clippy")
        .arg("--locked")
        .arg("--manifest-path")
        .arg(manifest_path.as_str())
        .arg("--bin")
        .arg(bin);
    Ok(command)
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
pub(super) fn prepare_ui_clippy() -> io::Result<nested_cargo::PreparationSummary> {
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
