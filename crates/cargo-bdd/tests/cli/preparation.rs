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

// The witness the family asserts on, and the withdrawal that makes a refused
// stamp recoverable. Split from this file for the line budget; declared here so
// the path resolves beside it.
#[path = "preparation/witness_gate.rs"]
mod witness_gate;

use witness_gate::witness_or_withdraw;

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

// The fixture's test targets are units of their own, so their eviction and the
// profile they are read from are pinned apart from the package cases above.
#[cfg(test)]
#[path = "preparation/tests/test_targets.rs"]
mod test_targets;

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

/// A budget long enough that no command in a regression can exhaust it.
///
/// The withdrawal regressions are about the witness and the stamp it refutes,
/// not about the budget, so they must not be able to fail because a machine was
/// slow. They still go through the same gate production does, which is the
/// point: a case that called the predicate directly would keep passing for a
/// gate that had stopped consulting it.
#[cfg(test)]
fn an_ample_budget() -> nested_cargo::Deadline {
    nested_cargo::Deadline::starting_now(Duration::from_secs(3600))
}

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
/// `deadline` is the budget the whole gate has, and is the caller's because its
/// two callers are not in the same position: see
/// [`closure::TIMED_GRAPH_QUERY_TIMEOUT`]. It is charged for the closure query
/// here and handed back so the caller can charge the identity probes against
/// what is left, rather than giving them a fresh allowance. A query that
/// succeeds just inside its own limit would otherwise spend the budget the
/// probes need and let the runner kill the test before either reported.
///
/// # Errors
///
/// Returns the environment, target-directory or input-hash error from
/// [`cli_preparation`], and the resolution error when the fixture's dependency
/// closure cannot be read or its query outlives the deadline.
fn cli_preparation_description(
    deadline: nested_cargo::Deadline,
    graph_query_timeout: Duration,
) -> Result<(
    nested_cargo::Preparation,
    nested_cargo::NestedCargoEnv,
    nested_cargo::Deadline,
)> {
    let (child_env, target_root, inputs) = cli_preparation()?;
    let fixture = fixture_dir();
    let mut preparation = nested_cargo::Preparation::new(
        nested_cargo::Sources::new("cargo-bdd-cli", env!("CARGO_PKG_VERSION"), inputs),
        nested_cargo::Directories::new(
            target_root,
            // The fixture's own directory, because that is where every nested
            // `cargo bdd steps` for this family runs. Cargo resolves a relative
            // `CARGO_HOME` and a relative `RUSTC` against the command's
            // directory, so recording the stamped target root instead would
            // have the identity probe read a different tree than the build.
            fixture,
        ),
        child_env.clone(),
    )?;
    // The query's own limit and the budget are both respected: the deadline
    // decides whether there is time to start, and the per-query bound keeps one
    // contended query from consuming the probes' share.
    let bound = deadline
        .cap(graph_query_timeout, "the fixture dependency closure")
        .wrap_err("the timed CLI gate has no budget left for its graph query")?;
    preparation.owned_packages = owned_units(closure::fixture_dependency_closure(bound)?);
    Ok((preparation, child_env, deadline))
}

/// The names the family's stamp speaks for: the closure and the test targets.
///
/// The closure names *packages*, and a package name covers only the units that
/// share it. The fixture's two integration tests are separate units named for
/// their targets — `deps/diagnostics_fixture-<hash>` rather than anything
/// carrying `rstest-bdd-cli-fixture` — so the closure alone filtered them out
/// of both the recorded and the present name lists. Removing a prepared test
/// binary then compared equal to itself, a second variant in the same profile
/// satisfied the witness, and the timed command relinked the missing one
/// inside its limit. Naming the targets makes their outputs part of the record.
fn owned_units(mut closure: Vec<String>) -> Vec<String> {
    closure.extend(witness::FIXTURE_TEST_BINARIES.map(str::to_owned));
    closure
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
    // One budget for the whole gate. The closure query and both identity
    // probes are charged against it, so no later command starts with an
    // allowance the earlier ones have already spent.
    let deadline = nested_cargo::Deadline::starting_now(closure::TIMED_GRAPH_QUERY_TIMEOUT);
    let (preparation, child_env, deadline) =
        cli_preparation_description(deadline, closure::TIMED_GRAPH_QUERY_TIMEOUT)?;
    ensure_gated_with(
        &preparation,
        nested_cargo::running_under_nextest(),
        deadline,
        || nested_build::capture_preparation(&child_env),
    )
}

/// The gate itself, with the family, the runner decision, the shared budget and
/// the preparation work supplied.
///
/// Split out so the refusal can be exercised against a synthetic tree without
/// this test needing the real one. A case that called the predicate below
/// directly would pass for a gate that had stopped consulting it, so the case
/// that matters drives *this* function — the same one every timed test reaches
/// through [`ensure_cli_prepared`].
///
/// The runner decision is a parameter for the reason
/// [`nested_cargo::ensure_prepared_building`] takes one: the two runners reach
/// opposite conclusions from a missing stamp, and no test can change which
/// runner it is in. The refusal under test lives on the `cargo test` branch,
/// where the preparation is allowed to run; under nextest the gate refuses
/// earlier, for a reason that says nothing about the witness.
///
/// The probes are the commands still to run by this point, so their bounds come
/// from what is left of `deadline`; the closure query has already been charged
/// by [`cli_preparation_description`].
fn ensure_gated_with<P>(
    preparation: &nested_cargo::Preparation,
    under_nextest: bool,
    deadline: nested_cargo::Deadline,
    prepare: P,
) -> Result<()>
where
    P: FnOnce() -> std::io::Result<nested_cargo::PreparationSummary>,
{
    nested_cargo::ensure_prepared_building_reported(
        preparation,
        under_nextest,
        nested_cargo::instruments_within(preparation, deadline),
        prepare,
    )
    .wrap_err("nested CLI fixture should be prepared")?;
    // A gate that admitted a stamp ran no preparation, so its count is zero by
    // construction — and it must still say so. See `record_reuse`.
    nested_build::record_reuse();
    witness_or_withdraw(preparation)
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
    // This test has its own nine-minute period rather than the timed
    // assertions' 180 s, so the budget it shares between the closure query and
    // the probes is the larger one. The deadline still applies: a query that
    // spent the whole preparation bound would leave the probes nothing, and the
    // runner would kill the test before either reported.
    let budget = nested_cargo::PREPARATION_COMMAND_TIMEOUT;
    let (preparation, child_env, deadline) =
        cli_preparation_description(nested_cargo::Deadline::starting_now(budget), budget)?;
    nested_cargo::prepare_now_reported(
        &preparation,
        nested_cargo::instruments_within(&preparation, deadline),
        || nested_build::capture_preparation(&child_env),
    )?;
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
