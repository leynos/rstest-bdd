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

/// The workspace root both fixtures build beneath.
fn workspace_root() -> std::path::PathBuf {
    std::path::PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../..")
}

/// The workspace inputs both fixtures compile against.
///
/// Each fixture declares path dependencies on `rstest-bdd` and
/// `rstest-bdd-macros`, so an edit to either crate changes what the fixture
/// builds while leaving the fixture tree untouched. Hashing them keeps the
/// stamp honest: without them the preparation would be skipped and the timed
/// `cargo test` would compile the changed crates inside its own bound.
///
/// The workspace's Cargo configuration is an input for the same reason. Cargo
/// discovers it by walking up from the nested build's working directory, so a
/// change to the development profile options or the Linux linker flags
/// invalidates dependency artefacts without touching a fixture source.
fn workspace_inputs() -> Vec<std::path::PathBuf> {
    let workspace = workspace_root();
    let mut roots: Vec<std::path::PathBuf> = ["Cargo.toml", "Cargo.lock", "crates"]
        .into_iter()
        .map(|entry| workspace.join(entry))
        .collect();
    roots.extend(nested_cargo::cargo_config_inputs(&workspace));
    roots
}

/// The directory inside the resolved target root where a family stamps.
///
/// Called with the target directory the child environment actually resolved,
/// never with a workspace default: an inherited `CARGO_TARGET_DIR` (a coverage
/// driver's, for instance) moves the artefacts, and a stamp left behind in the
/// workspace target would keep matching after those artefacts were evicted.
/// Under the default target this coincides with the harness's scratch root,
/// where the two stamps remain distinct files with distinct jobs: this one
/// records which artefacts exist, and `fixtures`'s `.stamp` records which
/// fixture source the scratch copy came from.
fn artefact_directory(target_directory: &std::path::Path, subdir: &str) -> std::path::PathBuf {
    target_directory.join("tests").join(subdir)
}

/// Describe one fixture's preparation family.
///
/// Both fixtures make up a `cargo test --no-run --locked --offline` closure
/// over the same workspace inputs, so only the family name, the artefact
/// subdirectory and the fixture source differ between them. Everything else —
/// the environment view and the variant label — is identical by construction
/// rather than by copy.
fn fixture_preparation(
    env: &process::ChildEnv,
    family: &str,
    artefact_subdir: &str,
    source_dir: std::path::PathBuf,
) -> std::io::Result<nested_cargo::Preparation> {
    let mut roots = vec![source_dir];
    roots.extend(workspace_inputs());
    let inputs = nested_cargo::input_hash(&roots)?;
    Ok(nested_cargo::Preparation::new(
        nested_cargo::Sources::new(family, "locked-offline-test-no-run", inputs),
        artefact_directory(process::child_target_directory(env), artefact_subdir),
        process::preparation_view(env),
    ))
}

/// Describe the rebuild fixture's preparation family.
fn rebuild_preparation(env: &process::ChildEnv) -> std::io::Result<nested_cargo::Preparation> {
    fixture_preparation(
        env,
        REBUILD_FAMILY,
        "rebuild-invalidation",
        fixtures::source_fixture_dir(),
    )
}

/// Describe the addition fixture's preparation family.
fn addition_preparation(env: &process::ChildEnv) -> std::io::Result<nested_cargo::Preparation> {
    fixture_preparation(
        env,
        ADDITION_FAMILY,
        "feature-addition",
        fixtures::source_addition_dir(),
    )
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

#[cfg(test)]
mod tests {
    //! Regression coverage for where the preparation stamp lives.
    //!
    //! A stamp is only meaningful if it is evicted together with the artefacts
    //! it vouches for. An inherited `CARGO_TARGET_DIR` moves those artefacts
    //! out of the workspace `target/`, so a stamp stored in the scratch root
    //! would survive an eviction of the external directory, keep matching, and
    //! let the timed assertion pay for a cold dependency compile — the exact
    //! failure issue 910 removes.

    use std::ffi::OsString;

    use rstest_bdd_harness::nested_cargo;

    use super::{REBUILD_FAMILY, fixture_preparation};
    use crate::harness::{fixtures, process};

    /// A child environment whose artefacts go to `target_directory`.
    fn child_env(target_directory: &std::path::Path) -> process::ChildEnv {
        process::child_env_from_vars(vec![(
            OsString::from("CARGO_TARGET_DIR"),
            target_directory.as_os_str().to_os_string(),
        )])
    }

    /// A scratch copy of the fixture, standing in for `fixtures::scratch_root`.
    fn scratch_fixture() -> tempfile::TempDir {
        let scratch = tempfile::tempdir().expect("scratch dir");
        let fixture = scratch.path().join("fixture");
        std::fs::create_dir_all(&fixture).expect("fixture dir");
        std::fs::write(fixture.join("Cargo.toml"), "[package]\nname = \"f\"\n").expect("manifest");
        scratch
    }

    #[test]
    fn an_inherited_target_directory_moves_the_stamp_with_the_artefacts() {
        let external = tempfile::tempdir().expect("external target dir");
        let env = child_env(external.path());
        assert_eq!(
            process::child_target_directory(&env),
            external.path(),
            "an inherited CARGO_TARGET_DIR must win over the workspace default"
        );

        let preparation = fixture_preparation(
            &env,
            REBUILD_FAMILY,
            "rebuild-invalidation",
            fixtures::source_fixture_dir(),
        )
        .expect("preparation description");

        assert!(
            preparation.directory.starts_with(external.path()),
            "the stamp must live inside the resolved target directory, not the scratch root: {}",
            preparation.directory.display()
        );
        assert_ne!(
            preparation.directory,
            fixtures::scratch_root(),
            "a stamp outside the external target would survive its eviction"
        );
    }

    #[test]
    fn without_an_inherited_target_the_workspace_target_is_used() {
        let env = process::child_env_from_vars(Vec::new());
        assert_eq!(
            process::child_target_directory(&env),
            fixtures::shared_target_dir()
        );
    }

    #[test]
    fn evicting_the_artefacts_invalidates_the_preparation() {
        // Prepare a family whose artefacts live outside the scratch tree, then
        // evict exactly that directory while leaving the scratch fixture alone.
        // The gate must refuse: the artefacts the stamp vouched for are gone,
        // and a timed assertion that trusted the stamp would rebuild the
        // dependency graph inside its own deadline.
        let external = tempfile::tempdir().expect("external target dir");
        let scratch = scratch_fixture();
        assert!(scratch.path().is_dir(), "the scratch fixture must survive");

        let env = child_env(external.path());
        let preparation = fixture_preparation(
            &env,
            REBUILD_FAMILY,
            "rebuild-invalidation",
            fixtures::source_fixture_dir(),
        )
        .expect("preparation description");
        // Guard before writing: a regression that stamped the shared scratch
        // root would otherwise leave a stamp in a directory this test does not
        // own.
        assert!(
            preparation.directory.starts_with(external.path()),
            "the stamp must live inside the resolved target directory: {}",
            preparation.directory.display()
        );
        nested_cargo::prepare_now(&preparation, || {
            Ok(nested_cargo::PreparationSummary::default())
        })
        .expect("preparation");
        nested_cargo::ensure_prepared_with(&preparation, true, || {
            panic!("a just-prepared family must satisfy the gate")
        })
        .expect("the freshly written stamp must be accepted");

        std::fs::remove_dir_all(process::child_target_directory(&env))
            .expect("evict the artefact directory");

        let error = nested_cargo::ensure_prepared_with(&preparation, true, || {
            panic!("a timed test must never prepare")
        })
        .expect_err("the evicted artefacts must fail the gate");
        assert_eq!(error.kind(), std::io::ErrorKind::NotFound);
        assert!(
            scratch.path().is_dir(),
            "the scratch fixture must be untouched"
        );
    }
}
