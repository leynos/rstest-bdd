//! Prerequisite compilation for the rebuild-invalidation experiments.
//!
//! Each experiment deliberately recompiles the fixture after mutating it: the
//! rebuild itself is the contract under test. What the experiments must not
//! do is compile the fixture's *dependencies* inside their own 300-second
//! bound, which is what happens when the shared `target/` has been evicted.
//! The downstream issue this module closes saw that cold graph build exhaust
//! the bound while Cargo waited in a healthy but busy compiler queue.
//!
//! Preparation therefore does exactly what the baselines would do first —
//! copy the scratch trees, restore the pristine feature file, remove
//! `zzz_added.feature`, write the documented `build.rs` — and then compiles
//! both fixtures' test binaries with `--locked --offline --no-run`. The
//! experiment that follows still cleans the fixture package, runs the
//! baseline, mutates the fixture and re-runs, in that order, so every
//! rebuild assertion keeps its meaning: only the fixture's own units are ever
//! cleaned, and only the fixture's own units are expected to recompile.
//!
//! Both fixtures live in separate scratch directories, so each family has its
//! own stamp inside its own root.

use rstest_bdd_harness::nested_cargo;

use super::{addition, fixtures, process};

/// Preparation family for the scratch rebuild-invalidation fixture.
const REBUILD_FAMILY: &str = "feature-rebuild-invalidation";

/// Preparation family for the scratch build-script addition fixture.
const ADDITION_FAMILY: &str = "feature-addition";

/// The `cargo` arguments that compile both fixture test binaries without
/// running them.
const PREPARE_ARGS: [&str; 5] = ["test", "--no-run", "--locked", "--offline", "--color=never"];

/// Prepare the rebuild-invalidation scratch fixture's dependency closure.
///
/// # Errors
///
/// Returns an I/O error carrying the captured child output when the scratch
/// copy, the metadata preflight or the compile fails, and the stamp I/O error
/// when the stamp cannot be written.
pub(crate) fn ensure_rebuild_prepared() -> std::io::Result<()> {
    let env = process::build_child_env();
    let preparation = rebuild_preparation(&env)?;
    nested_cargo::ensure_prepared(&preparation, || prepare_rebuild(&env))
}

/// Prepare both experiments unconditionally, for the named preparation test.
///
/// # Errors
///
/// As [`ensure_rebuild_prepared`], and with any error from preparing the
/// addition fixture.
pub(crate) fn prepare_all() -> std::io::Result<()> {
    let env = process::build_child_env();
    let rebuild = rebuild_preparation(&env)?;
    nested_cargo::prepare_now(&rebuild, || prepare_rebuild(&env))?;
    let addition = addition_preparation(&env)?;
    nested_cargo::prepare_now(&addition, || prepare_addition(&env))
}

/// Prepare the build-script addition scratch fixture's dependency closure.
///
/// # Errors
///
/// As [`ensure_rebuild_prepared`].
pub(crate) fn ensure_addition_prepared() -> std::io::Result<()> {
    let env = process::build_child_env();
    let preparation = addition_preparation(&env)?;
    nested_cargo::ensure_prepared(&preparation, || prepare_addition(&env))
}

/// The workspace inputs both fixtures compile against.
///
/// Each fixture declares path dependencies on `rstest-bdd` and
/// `rstest-bdd-macros`, so an edit to either crate changes what the fixture
/// builds while leaving the fixture tree untouched. Hashing them keeps the
/// stamp honest: without them the preparation would be skipped and the timed
/// `cargo test` would compile the changed crates inside its own bound.
fn workspace_inputs() -> Vec<std::path::PathBuf> {
    let workspace = std::path::PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../..");
    ["Cargo.toml", "Cargo.lock", "crates"]
        .into_iter()
        .map(|entry| workspace.join(entry))
        .collect()
}

/// Describe the rebuild fixture's preparation family.
fn rebuild_preparation(env: &process::ChildEnv) -> std::io::Result<nested_cargo::Preparation> {
    let mut roots = vec![fixtures::source_fixture_dir()];
    roots.extend(workspace_inputs());
    let inputs = nested_cargo::input_hash(&roots)?;
    Ok(nested_cargo::Preparation::new(
        REBUILD_FAMILY,
        fixtures::scratch_root(),
        process::preparation_view(env),
        "locked-offline-test-no-run",
        inputs,
    ))
}

/// Describe the addition fixture's preparation family.
fn addition_preparation(env: &process::ChildEnv) -> std::io::Result<nested_cargo::Preparation> {
    let mut roots = vec![fixtures::source_addition_dir()];
    roots.extend(workspace_inputs());
    let inputs = nested_cargo::input_hash(&roots)?;
    Ok(nested_cargo::Preparation::new(
        ADDITION_FAMILY,
        fixtures::add_addition_root(),
        process::preparation_view(env),
        "locked-offline-test-no-run",
        inputs,
    ))
}

/// Copy the scratch trees and compile the rebuild fixture's test binaries.
fn prepare_rebuild(env: &process::ChildEnv) -> std::io::Result<nested_cargo::PreparationSummary> {
    fixtures::ensure_fixture_copied();
    fixtures::restore_feature_file();
    let captured = run_prepared(env, &fixtures::scratch_fixture_dir())?;
    Ok(captured)
}

/// Copy and prime the addition scratch tree, then compile its test binaries.
fn prepare_addition(env: &process::ChildEnv) -> std::io::Result<nested_cargo::PreparationSummary> {
    fixtures::ensure_addition_fixture_copied();
    addition::reset_added_file();
    addition::write_build_script_from_docs().map_err(|err| {
        std::io::Error::other(format!("cannot prime the addition fixture: {err}"))
    })?;
    let preflight = {
        let mut cmd = process::cargo_command(env, &fixtures::scratch_addition_dir());
        cmd.args(["metadata", "--locked", "--offline", "--format-version", "1"]);
        process::run_preparation_bounded(&mut cmd)?
    };
    if !preflight.status.success() {
        return Err(std::io::Error::other(format!(
            "manifest-lockfile consistency preflight failed for the prepared addition fixture:\n{}",
            String::from_utf8_lossy(&preflight.stderr)
        )));
    }
    run_prepared(env, &fixtures::scratch_addition_dir())
}

/// Compile one scratch fixture's test binaries under the preparation bound.
fn run_prepared(
    env: &process::ChildEnv,
    cwd: &std::path::Path,
) -> std::io::Result<nested_cargo::PreparationSummary> {
    let mut cmd = process::cargo_command(env, cwd);
    cmd.args(PREPARE_ARGS);
    let output = process::run_preparation_bounded(&mut cmd)?;
    if !output.status.success() {
        return Err(std::io::Error::other(format!(
            "preparation compile failed in {}:\nstdout:\n{}\nstderr:\n{}",
            cwd.display(),
            String::from_utf8_lossy(&output.stdout),
            String::from_utf8_lossy(&output.stderr),
        )));
    }
    Ok(nested_cargo::PreparationSummary {
        compiled_units: nested_cargo::count_compiling(&output.stderr),
    })
}
