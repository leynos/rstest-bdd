//! Preparation of the CLI fixture's nested Cargo build.
//!
//! The timed CLI smoke tests assert on `cargo-bdd`'s diagnostic output; the
//! dependency graph they used to compile inside their own limits is built here
//! instead, by the `nested_fixture_preparation` test that
//! `.config/nextest.toml` runs first with its own budget. The timed tests gate
//! on [`ensure_cli_prepared`], which under nextest fails fast with the
//! preparation command rather than paying for the cold build itself.

use std::{env, path::PathBuf};

use eyre::{Context, Result};
use rstest_bdd_harness::nested_cargo;
use serial_test::serial;

use super::{cli_child_environment, fixture_dir, locate_or_build_cargo_bdd_command};

/// The directories and fingerprint that describe this suite's prepared
/// artefacts.
///
/// The prepared work is `cargo-bdd` itself plus the fixture workspace's test
/// binaries, built into the target root the CLI child environment resolves.
/// Both the binary's own output directory (`<target>/debug`) and the nested
/// workspace builds live under that root, so one stamp covers them.
fn cli_preparation() -> Result<(nested_cargo::NestedCargoEnv, PathBuf, String)> {
    let child_env = cli_child_environment(env::vars_os().collect());
    let target_root = child_env
        .get("CARGO_TARGET_DIR")
        .map(PathBuf::from)
        .ok_or_else(|| eyre::eyre!("CLI child environment must define CARGO_TARGET_DIR"))?;
    // The whole fixture tree is an input: its manifest and lockfile fix the
    // dependency resolution, and its sources and feature files decide what the
    // nested workspace builds. `input_hash` skips any `target` directory.
    //
    // The workspace manifest, lockfile and `crates/` tree are inputs too. The
    // fixture declares path dependencies on `rstest-bdd` and
    // `rstest-bdd-macros`, so editing either crate changes what the nested
    // workspace compiles while leaving the fixture tree untouched; without them
    // the stamp would survive such an edit and the timed test would rebuild the
    // changed crates inside its own limit.
    let workspace = super::workspace_root();
    let inputs = nested_cargo::input_hash(&[
        fixture_dir(),
        workspace.join("Cargo.toml"),
        workspace.join("Cargo.lock"),
        workspace.join("crates"),
    ])?;
    Ok((child_env, target_root, inputs))
}

/// Describe the CLI preparation family.
fn cli_preparation_description() -> Result<(nested_cargo::Preparation, nested_cargo::NestedCargoEnv)>
{
    let (child_env, target_root, inputs) = cli_preparation()?;
    let preparation = nested_cargo::Preparation::new(
        nested_cargo::Sources::new("cargo-bdd-cli", env!("CARGO_PKG_VERSION"), inputs),
        target_root,
        child_env.clone(),
    );
    Ok((preparation, child_env))
}

/// Run one diagnostic against the fixture, returning the captured output.
///
/// The child gets the filtered environment and an unfiltered inherited one for
/// `cargo`'s own diagnostics, matching how the timed tests spawn it except
/// that the output is bounded and captured.
fn capture_cargo_bdd_steps(
    child_env: &nested_cargo::NestedCargoEnv,
) -> Result<nested_cargo::CapturedOutput> {
    let fixture_dir = fixture_dir();
    let program = locate_or_build_cargo_bdd_command()?;
    let mut cmd = nested_cargo::filtered_command(
        std::path::Path::new(program.get_program()),
        child_env,
        &fixture_dir,
    );
    cmd.arg("steps");
    let output =
        nested_cargo::run_bounded_with_timeout(&mut cmd, nested_cargo::PREPARATION_COMMAND_TIMEOUT)
            .wrap_err("running `cargo bdd steps` for fixture preparation")?;
    Ok(output.into())
}

/// Capture the preparation summary, failing on a non-zero child status.
fn capture_preparation(
    child_env: &nested_cargo::NestedCargoEnv,
) -> std::io::Result<nested_cargo::PreparationSummary> {
    let captured = capture_cargo_bdd_steps(child_env)
        .map_err(|error| std::io::Error::other(format!("{error:#}")))?;
    if !captured.status {
        return Err(std::io::Error::other(format!(
            "`cargo bdd steps` failed during fixture preparation:\n{}",
            captured.combined()
        )));
    }
    Ok(nested_cargo::PreparationSummary {
        compiled_units: nested_cargo::count_compiling(captured.stderr.as_bytes())
            + nested_cargo::count_compiling(captured.stdout.as_bytes()),
    })
}

/// Gate a timed CLI test on the preparation test having run.
pub(super) fn ensure_cli_prepared() -> Result<()> {
    let (preparation, child_env) = cli_preparation_description()?;
    nested_cargo::ensure_prepared(&preparation, || capture_preparation(&child_env))
        .wrap_err("nested CLI fixture should be prepared")
}

/// Prepare the CLI fixture's nested build before any timed assertion.
///
/// The suite's timed tests only assert on the diagnostic output; the cold
/// dependency compile they used to pay for now belongs to this test, which
/// `.config/nextest.toml` runs first with its own budget.
#[test]
#[serial]
fn nested_fixture_preparation() {
    let (preparation, child_env) =
        cli_preparation_description().expect("CLI preparation description should resolve");
    nested_cargo::prepare_now(&preparation, || capture_preparation(&child_env))
        .expect("nested CLI fixture preparation should succeed");
}
