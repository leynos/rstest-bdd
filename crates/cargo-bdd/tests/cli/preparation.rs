//! Preparation of the CLI fixture's nested Cargo build.
//!
//! The timed CLI smoke tests assert on `cargo-bdd`'s diagnostic output; the
//! dependency graph they used to compile inside their own limits is built here
//! instead, by the `nested_fixture_preparation` test that
//! `.config/nextest.toml` runs first with its own budget. The timed tests gate
//! on [`ensure_cli_prepared`], which under nextest fails fast with the
//! preparation command rather than paying for the cold build itself.

use std::{env, path::PathBuf, time::Duration};

use eyre::{Context, Result};
use rstest_bdd_harness::nested_cargo;
use serial_test::serial;

use super::{cli_child_environment, fixture_dir};

#[path = "witness.rs"]
mod witness;

// The nested build and the evidence it files. Split out because the reporting
// contract — marker, variable name and line format — is a second concern beside
// the family description and its gate, and belongs where a reader looking for
// it will find it.
#[path = "preparation/nested_build.rs"]
mod nested_build;

// The withdrawal regression lives in a submodule of its own so this file
// keeps its share of the repository's line budget. Declared here rather than
// inside `tests` so the path resolves beside this file.
#[cfg(test)]
#[path = "preparation/tests/withdrawal.rs"]
mod withdrawal;

// The report lines the gate and the timed command file are a contract with a
// reader in another language, so they are pinned against that grammar rather
// than only observed in a cold end-to-end run.
#[cfg(test)]
#[path = "preparation/tests/report.rs"]
mod report;

// The ownership regression drives the declaration below rather than a
// literal, so it lives beside it for the same reason.
#[cfg(test)]
#[path = "preparation/tests/ownership.rs"]
mod ownership;

use witness::fixture_binaries_present;

/// File the compilations the timed `cargo bdd steps` paid for.
///
/// The gate's [`nested_build::record_reuse`] reports a different fact — that no
/// *preparation* ran — so it cannot stand as evidence about the timed command,
/// which runs afterwards. This is the only place that command's captured stderr
/// is still in hand, so the count has to be filed from here.
pub(super) fn record_timed_compilations(stderr: &str) {
    nested_build::record_timed(nested_cargo::count_compiling(stderr.as_bytes()));
}

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
fn family_inputs() -> Result<String> { Ok(nested_cargo::input_hash(&family_roots()?)?) }

/// The roots [`family_inputs`] hashes.
///
/// Split out so the declaration regression can remove exactly the committed
/// workspace Cargo configuration from the family's *own* list and hash both
/// sides. A case that restated the list instead would compare the family
/// against a copy of itself and pass for a family that had stopped naming a
/// real input.
///
/// # Errors
///
/// Returns the error when the Cargo configuration cannot be inspected.
pub(super) fn family_roots() -> Result<Vec<PathBuf>> {
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
    Ok(roots)
}

// The fixture's resolved dependency closure, measured from the fixture.
// Declared here rather than beside its use because `tests` reaches it by the
// same path the family description does.
#[path = "preparation/closure.rs"]
mod closure;

/// Cargo package name of the CLI fixture crate.
const FIXTURE_PACKAGE: &str = "rstest-bdd-cli-fixture";

/// Describe the CLI preparation family.
///
/// The family names its fixture package's whole dependency closure because it
/// builds into the workspace `target/debug`, which the integration-test
/// binaries and the rebuild scratch families also write to. A stamp that
/// recorded the whole profile would vouch for units this family never built —
/// and the rebuild suite's deliberate `cargo clean -p` would then leave this
/// stamp refused for ever. Naming the closure is the opposite correction: those
/// units *were* built here, and leaving them out would let their eviction pass.
///
/// `graph_query_timeout` bounds the closure query, and is the caller's because
/// its two callers are not in the same position: see
/// [`closure::TIMED_GRAPH_QUERY_TIMEOUT`].
///
/// # Errors
///
/// Returns the environment, target-directory or input-hash error from
/// [`cli_preparation`], and the resolution error when the fixture's dependency
/// closure cannot be read.
fn cli_preparation_description(
    graph_query_timeout: Duration,
) -> Result<(nested_cargo::Preparation, nested_cargo::NestedCargoEnv)> {
    let (child_env, target_root, inputs) = cli_preparation()?;
    let fixture = fixture_dir();
    let mut preparation = nested_cargo::Preparation::new(
        nested_cargo::Sources::new("cargo-bdd-cli", env!("CARGO_PKG_VERSION"), inputs),
        target_root,
        child_env.clone(),
        // The fixture's own directory, because that is where every nested
        // `cargo bdd steps` for this family runs. Cargo resolves a relative
        // `CARGO_HOME` and a relative `RUSTC` against the command's directory,
        // so recording the stamped target root instead would have the identity
        // probe read a different tree than the build.
        fixture,
    )?;
    preparation.owned_packages = closure::fixture_dependency_closure(graph_query_timeout)?;
    Ok((preparation, child_env))
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
    let (preparation, child_env) = cli_preparation_description(closure::TIMED_GRAPH_QUERY_TIMEOUT)?;
    ensure_gated(&preparation, || {
        nested_build::capture_preparation(&child_env)
    })
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
    // A gate that admitted a stamp ran no preparation, so its count is zero by
    // construction — and it must still say so. See `record_reuse`.
    nested_build::record_reuse();
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

/// Fail unless `preparation`'s profiles hold the fixture's own test binaries.
///
/// Shared by the timed gate and the named preparation test so the two cannot
/// disagree about what "prepared" means: the predicate the preparation proves
/// is the one the timed assertions require.
///
/// Every profile the family names is searched, and any one holding both
/// binaries satisfies the witness. The profile the fixture's units land in is
/// not always the first: a build selected onto a target triple writes the
/// fixture's units to `<target>/<triple>/debug` and only its build scripts and
/// proc macros to `<target>/debug`, so a witness reading one named directory
/// would refuse a preparation that left exactly the binaries it asks for. The
/// family names both profiles rather than this deciding which one *should*
/// hold them, because only Cargo knows where a given selection puts them.
///
/// The search stops at the first profile that holds them, and a profile that
/// cannot be listed is remembered rather than raised immediately: in a
/// `--target` build the host profile is named first and may be unreadable while
/// the triple's profile holds exactly what the witness asks for. Failing on the
/// host profile before looking at the second would refuse a prepared tree.
///
/// # Errors
///
/// Returns the listing error when no profile could be read at all, and an error
/// naming every profile searched when each one was readable and missing a
/// fixture binary.
fn require_fixture_binaries(preparation: &nested_cargo::Preparation) -> Result<()> {
    let profiles = &preparation.artefacts;
    if profiles.is_empty() {
        return Err(eyre::eyre!(
            "the CLI family must describe the profile its units land in"
        ));
    }
    let mut listing_error: Option<eyre::Error> = None;
    for profile in profiles {
        match fixture_binaries_present(profile) {
            Ok(true) => return Ok(()),
            Ok(false) => {}
            Err(error) => {
                let error = eyre::Report::new(error).wrap_err(format!(
                    "listing the prepared profile {}",
                    profile.display()
                ));
                if listing_error.is_none() {
                    listing_error = Some(error);
                }
            }
        }
    }
    if let Some(error) = listing_error {
        return Err(error);
    }
    let searched: Vec<String> = profiles
        .iter()
        .map(|profile| profile.display().to_string())
        .collect();
    Err(eyre::eyre!(
        "preparation must leave the fixture's own test binaries in one of {}; the timed CLI \
         assertions read them, and a preparation that compiled nothing leaves the suite with \
         nothing to inspect",
        searched.join(", ")
    ))
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
    let (preparation, child_env) =
        cli_preparation_description(nested_cargo::PREPARATION_COMMAND_TIMEOUT)?;
    nested_cargo::prepare_now(&preparation, || {
        nested_build::capture_preparation(&child_env)
    })?;
    // Through the same function the timed gate uses, so a preparation that
    // leaves no binaries is refused *and* has its stamp withdrawn here too.
    // This test runs before the timed assertions under nextest, so a stamp it
    // left behind would be the one they reuse.
    witness_or_withdraw(&preparation)
}

/// Regression coverage for the prepared CLI fixture.
///
/// Declared beside `withdrawal` and `ownership` rather than inline, so this file
/// keeps its share of the repository's line budget.
#[cfg(test)]
#[path = "preparation/tests/declaration.rs"]
mod declaration;
