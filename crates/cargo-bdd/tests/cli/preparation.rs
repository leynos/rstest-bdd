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

#[path = "witness.rs"]
mod witness;

use witness::fixture_binaries_present;

/// The directories and fingerprint that describe this suite's prepared
/// artefacts.
///
/// The prepared work is `cargo-bdd` itself plus the fixture workspace's test
/// binaries, built into the target root the CLI child environment resolves.
/// Both the binary's own output directory (`<target>/debug`) and the nested
/// workspace builds live under that root, so one stamp covers them.
fn cli_preparation() -> Result<(nested_cargo::NestedCargoEnv, PathBuf, String)> {
    let child_env = cli_child_environment(env::vars_os().collect())?;
    // Resolved through the shared helper, not by parsing the variable here: a
    // relative `CARGO_TARGET_DIR` names one directory for this process and a
    // different one for the nested build, which runs in the fixture directory.
    // The helper turns it into an absolute path once, so the stamp and the
    // artefacts agree.
    let target_root = child_env
        .target_directory()?
        .ok_or_else(|| eyre::eyre!("CLI child environment must define CARGO_TARGET_DIR"))?;
    Ok((child_env, target_root, family_inputs()?))
}

/// The hash of everything the CLI fixture family's build depends on.
///
/// Shared by the timed family and the isolated one so the two cannot disagree
/// about what makes this fixture stale.
///
/// # Errors
///
/// Returns the hashing error when a root cannot be read.
fn family_inputs() -> Result<String> {
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
    let mut roots = vec![
        fixture_dir(),
        workspace.join("Cargo.toml"),
        workspace.join("Cargo.lock"),
        workspace.join("crates"),
    ];
    // Cargo discovers the workspace configuration from the nested build's
    // working directory, so the development profile options and the Linux
    // linker flags in it change dependency artefacts without touching a
    // fixture source.
    roots.extend(nested_cargo::cargo_config_inputs(&workspace)?);
    Ok(nested_cargo::input_hash(&roots)?)
}

/// Describe the CLI preparation family.
fn cli_preparation_description() -> Result<(nested_cargo::Preparation, nested_cargo::NestedCargoEnv)>
{
    let (child_env, target_root, inputs) = cli_preparation()?;
    let preparation = nested_cargo::Preparation::new(
        nested_cargo::Sources::new("cargo-bdd-cli", env!("CARGO_PKG_VERSION"), inputs),
        target_root,
        child_env.clone(),
    )?;
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
///
/// It also carries the regression this family exists to protect: preparation
/// must leave the fixture's own test binaries in the profile the timed tests
/// read. Asserting it here rather than in a test of its own is deliberate.
/// Building the fixture into a *fresh* target would discriminate a no-op from a
/// real preparation more sharply, and would reintroduce exactly the cold
/// dependency build this change moves out of timed tests: a fresh target means
/// the whole graph. On this test's target the binaries may already be warm, so
/// the assertion proves the family's artefacts are complete rather than proving
/// who produced them; the predicate that distinguishes a no-op is checked
/// separately, against output that compiled nothing.
#[test]
#[serial]
fn nested_fixture_preparation() -> Result<()> {
    let (preparation, child_env) = cli_preparation_description()?;
    nested_cargo::prepare_now(&preparation, || capture_preparation(&child_env))?;

    let profile = preparation
        .artefacts
        .first()
        .ok_or_else(|| eyre::eyre!("the CLI family must describe the profile its units land in"))?;
    let present = fixture_binaries_present(profile)
        .wrap_err_with(|| format!("listing the prepared profile {}", profile.display()))?;
    if !present {
        return Err(eyre::eyre!(
            "preparation must leave the fixture's own test binaries in {}; the timed CLI \
             assertions read them, and a preparation that compiled nothing leaves the suite with \
             nothing to inspect",
            profile.display()
        ));
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    //! Regression coverage for the prepared CLI fixture.
    //!
    //! The family's input hash includes the workspace's Cargo configuration, so
    //! a profile or linker change forces preparation instead of being absorbed
    //! by a warm stamp. And the witness the preparation test asserts on — the
    //! fixture's own test binaries in the profile — is shown to reject a tree
    //! that holds unrelated artefacts instead, because a check that only ever
    //! runs against a warm tree cannot distinguish a real preparation from a
    //! no-op.

    use super::{cli_preparation, fixture_dir, nested_cargo};
    use crate::workspace_root;

    /// The CLI family's inputs without the workspace Cargo configuration.
    ///
    /// Kept beside the assertion so a future change to the family's roots makes
    /// this list visibly stale rather than silently passing.
    fn inputs_without_cargo_configuration() -> Vec<std::path::PathBuf> {
        let workspace = workspace_root();
        vec![
            fixture_dir(),
            workspace.join("Cargo.toml"),
            workspace.join("Cargo.lock"),
            workspace.join("crates"),
        ]
    }

    #[test]
    fn the_workspace_cargo_configuration_is_an_input() {
        // The fixture build runs beneath the workspace, so Cargo applies the
        // committed `.cargo/config.toml`: its development profile options and
        // Linux linker flags rebuild dependency artefacts without changing a
        // fixture source. A stamp that ignored it would survive such an edit and
        // the timed CLI test would compile the graph inside its own 180 s limit.
        let (_environment, _target_root, inputs) =
            cli_preparation().expect("preparation description should resolve");
        let without = nested_cargo::input_hash(&inputs_without_cargo_configuration())
            .expect("hash without the workspace configuration");
        assert_ne!(
            inputs, without,
            "the workspace Cargo configuration must change the CLI input hash"
        );
    }
}
