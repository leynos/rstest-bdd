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

// The withdrawal regression lives in a submodule of its own so this file
// keeps its share of the repository's line budget. Declared here rather than
// inside `tests` so the path resolves beside this file.
#[cfg(test)]
#[path = "preparation/tests/withdrawal.rs"]
mod withdrawal;

// The ownership regression drives the declaration below rather than a
// literal, so it lives beside it for the same reason.
#[cfg(test)]
#[path = "preparation/tests/ownership.rs"]
mod ownership;

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

/// Cargo package name of the CLI fixture crate.
const FIXTURE_PACKAGE: &str = "rstest-bdd-cli-fixture";

/// The workspace crates preparing this fixture compiles.
///
/// Preparing this family builds them, so its stamp has to vouch for their
/// units: Cargo rebuilds a dependency whose fingerprint record is gone, and it
/// would do so inside whichever timed assertion ran next. They are outside
/// `FIXTURE_PACKAGE`, so declaring the fixture alone leaves them filtered out
/// of both the recorded and the present name lists, where an evicted record
/// then compares equal to itself and passes the gate.
///
/// The list is the fixture's *whole* workspace dependency closure, not only the
/// two crates its manifest names. `rstest-bdd` depends on `rstest-bdd-patterns`
/// and `rstest-bdd-policy`, and `rstest-bdd-macros` on those two and
/// `rstest-bdd-harness`, so one build also compiles four crates the manifest
/// never mentions. Naming only the direct edges left a transitive dependency's
/// eviction passing exactly as the fixture's own did before, which is the same
/// defect one level down.
///
/// Measured from the fixture's own graph rather than assumed. The unrestricted
/// closure, `cargo tree --prefix none` in `tests/fixtures/minimal`, names every
/// one of them: `rstest-bdd`, `rstest-bdd-macros`, `rstest-bdd-harness`,
/// `rstest-bdd-patterns` and `rstest-bdd-policy`. The narrower
/// `--edges normal,build` view is *not* the answer here — it omits the two
/// proc-macro host units, and a first version of this list was written from it
/// and was wrong for exactly that reason.
///
/// A crate the graph gains later is not covered by this list, and nothing in
/// the tree can tell a new dependency from a sibling's unit at check time; that
/// is why `Preparation::owned_packages` documents that the caller declares it.
const FIXTURE_PATH_DEPENDENCIES: &[&str] = &[
    "rstest-bdd",
    "rstest-bdd-macros",
    "rstest-bdd-harness",
    "rstest-bdd-patterns",
    "rstest-bdd-policy",
];

/// Describe the CLI preparation family.
///
/// The family names its fixture package because it builds into the workspace
/// `target/debug`, which the integration-test binaries and the rebuild
/// scratch families also write to. A stamp that recorded the whole profile
/// would vouch for units this family never built — and the rebuild suite's
/// deliberate `cargo clean -p` would then leave this stamp refused for ever.
/// It names the fixture's path dependencies for the opposite reason: those
/// units *were* built here, and leaving them out would let their eviction pass.
fn cli_preparation_description() -> Result<(nested_cargo::Preparation, nested_cargo::NestedCargoEnv)>
{
    let (child_env, target_root, inputs) = cli_preparation()?;
    let mut preparation = nested_cargo::Preparation::new(
        nested_cargo::Sources::new("cargo-bdd-cli", env!("CARGO_PKG_VERSION"), inputs),
        target_root,
        child_env.clone(),
    )?;
    // The fixture first, then the crates its build pulls in. Built by chaining
    // rather than by a conditional append so the list is never empty on this
    // path: an empty `owned_packages` means "claims the whole profile", which
    // is exactly what this family must not do.
    preparation.owned_packages = std::iter::once(FIXTURE_PACKAGE)
        .chain(FIXTURE_PATH_DEPENDENCIES.iter().copied())
        .map(str::to_owned)
        .collect();
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
///
/// The stamp is not the whole gate. It answers "a preparation ran for these
/// inputs", and it is written the moment the preparation commands succeed,
/// which is *before* this family's own witness can run: the witness reads the
/// binaries the preparation was supposed to leave, so it cannot precede it.
/// A preparation that exits zero while leaving them out would therefore leave a
/// stamp behind, and a stamp-only gate would admit every timed assertion after
/// it against a tree with nothing to inspect. Re-asserting the witness here is
/// what makes the gate and the preparation test agree: a stamp whose tree does
/// not hold the fixture's own test binaries is refused wherever it is read,
/// not merely where it was written.
///
/// This also covers the reused-stamp path. `prepare_now` returns early from a
/// matching stamp without running the preparation at all, so the preparation
/// test's witness cannot run there either; this check does.
pub(super) fn ensure_cli_prepared() -> Result<()> {
    let (preparation, child_env) = cli_preparation_description()?;
    ensure_gated(&preparation, || capture_preparation(&child_env))
}

/// The gate itself, with the family and the preparation work supplied.
///
/// Split out so the refusal can be exercised against a synthetic tree without
/// this test needing the real one. A case that called the predicate below
/// directly would pass for a gate that had stopped consulting it, so the case
/// that matters drives *this* function — the same one every timed test reaches
/// through [`ensure_cli_prepared`].
fn ensure_gated<P>(preparation: &nested_cargo::Preparation, prepare: P) -> Result<()>
where
    P: FnOnce() -> std::io::Result<nested_cargo::PreparationSummary>,
{
    ensure_gated_with(preparation, nested_cargo::running_under_nextest(), prepare)
}

/// [`ensure_gated`] with the runner decision supplied.
///
/// The decision is a parameter for the reason [`nested_cargo::ensure_prepared_with`]
/// takes one: the two runners reach opposite conclusions from a missing stamp,
/// and no test can change which runner it is in. The refusal under test lives
/// on the `cargo test` branch, where the preparation is allowed to run; under
/// nextest the gate refuses earlier, for a reason that says nothing about the
/// witness.
fn ensure_gated_with<P>(
    preparation: &nested_cargo::Preparation,
    under_nextest: bool,
    prepare: P,
) -> Result<()>
where
    P: FnOnce() -> std::io::Result<nested_cargo::PreparationSummary>,
{
    nested_cargo::ensure_prepared_with(preparation, under_nextest, prepare)
        .wrap_err("nested CLI fixture should be prepared")?;
    witness_or_withdraw(preparation)
}

/// Refuse a preparation whose profile lacks the fixture binaries, and withdraw
/// the stamp that admitted it.
///
/// Both callers reach a preparation through a gate that returns early from a
/// *matching* stamp: [`nested_cargo::ensure_prepared_with`] on the lazy path and
/// [`nested_cargo::prepare_now`] in the named test. The stamp is written by the
/// preparation, before this witness can be read, so a preparation that exits
/// zero while leaving the fixture's own units out leaves a stamp vouching for a
/// tree no timed assertion can use. Reporting that alone would make every later
/// attempt read the same stamp and fail the same way, with the preparation never
/// running again — the failure would be permanent rather than transient.
///
/// Removing the stamp first is what makes the next attempt a build. The
/// removal's outcome is reported alongside the refusal rather than instead of
/// it: the missing binaries are the failure, and a stamp that could not be
/// withdrawn is a second fact the reader needs, not a substitute for the first.
fn witness_or_withdraw(preparation: &nested_cargo::Preparation) -> Result<()> {
    let error = match require_fixture_binaries(preparation) {
        Ok(()) => return Ok(()),
        Err(error) => error,
    };
    let recovery = match nested_cargo::invalidate(preparation) {
        Ok(()) => "the stamp was withdrawn, so the next run prepares again".to_owned(),
        Err(removal) => format!(
            "the stamp could not be withdrawn, so the next run will repeat this failure: {removal}"
        ),
    };
    Err(eyre::eyre!("{error:#}\n{recovery}"))
}

/// Fail unless `preparation`'s profile holds the fixture's own test binaries.
///
/// Shared by the timed gate and the named preparation test so the two cannot
/// disagree about what "prepared" means: the predicate the preparation proves
/// is the one the timed assertions require.
///
/// # Errors
///
/// Returns an error naming the profile when a fixture binary is missing, and
/// the listing error when the profile itself cannot be read.
fn require_fixture_binaries(preparation: &nested_cargo::Preparation) -> Result<()> {
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
/// this assertion alone proves the family's artefacts are complete rather than
/// proving who produced them.
///
/// What closes that gap is that the same predicate is part of
/// [`ensure_cli_prepared`], which every timed test calls. A preparation that
/// exits zero and leaves no binaries still leaves a stamp — the stamp is
/// written by the preparation, before any witness can read what it produced —
/// so a stamp-only gate would admit the timed assertions against an empty
/// tree. Applying the witness on the read path too is what makes a no-op fail
/// wherever the stamp came from, including from a previous run.
#[test]
#[serial]
fn nested_fixture_preparation() -> Result<()> {
    let (preparation, child_env) = cli_preparation_description()?;
    nested_cargo::prepare_now(&preparation, || capture_preparation(&child_env))?;
    // Through the same function the timed gate uses, so a preparation that
    // leaves no binaries is refused *and* has its stamp withdrawn here too.
    // This test runs before the timed assertions under nextest, so a stamp it
    // left behind would be the one they reuse.
    witness_or_withdraw(&preparation)
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
