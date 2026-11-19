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

/// Cargo package name of the rebuild-invalidation fixture.
///
/// Declared so the preparation can name the package its experiment cleans, and
/// is deliberately **not** part of [`Family::package`]'s ownership claim. See
/// [`owned_packages`] for why a family cannot vouch for the very units its
/// experiment removes.
const REBUILD_PACKAGE: &str = "rstest-bdd-rebuild-invalidation-fixture";

/// Cargo package name of the feature-addition fixture.
///
/// See [`REBUILD_PACKAGE`].
const ADDITION_PACKAGE: &str = "rstest-bdd-feature-addition-fixture";

/// The workspace crates both fixtures build through their path dependencies.
///
/// `PREPARE_ARGS` compiles each fixture's whole closure, not only the fixture:
/// `rstest-bdd` is a normal path dependency and `rstest-bdd-macros` a
/// dev-dependency, and each pulls the three harness-side crates in turn.
/// [`nested_cargo::Preparation::owned_packages`] requires every package the
/// preparation built, and declaring only the fixture would filter a
/// dependency's records out of both the recorded and the present lists — so an
/// evicted dependency would compare equal to itself and pass the gate, leaving
/// the timed rebuild to compile it. Measured from the fixtures rather than
/// assumed: `cargo tree --prefix none` in either fixture directory names all
/// five. The narrower `--edges normal,build` view is *not* the answer, because
/// it omits the two proc-macro host units.
const WORKSPACE_DEPENDENCIES: &[&str] = &[
    "rstest-bdd",
    "rstest-bdd-macros",
    "rstest-bdd-harness",
    "rstest-bdd-patterns",
    "rstest-bdd-policy",
];

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
    let env = process::build_child_env()?;
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
    let env = process::build_child_env()?;
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
    let env = process::build_child_env()?;
    let preparation = addition_preparation(&env)?;
    nested_cargo::ensure_prepared(&preparation, || prepare_addition(&env))
}

/// The workspace root both fixtures build beneath.
///
/// Resolved once here rather than at each comparison. [`workspace_inputs`]
/// mixes this root's own entries with [`nested_cargo::cargo_config_inputs`],
/// which canonicalizes the root it is given and reports the workspace
/// configuration through that resolved form. Holding the lexical spelling
/// (`crates/rstest-bdd/../..`) beside a resolved one would make the two
/// disagree about a single directory — on Windows the resolved form carries
/// the `\\?\` verbatim prefix, which no plain-spelled prefix can match — and a
/// test asking whether the configuration is among the family's inputs would
/// answer no while the file sat in the list. Resolving at the definition means
/// every caller compares in one form and no caller has to remember to.
fn workspace_root() -> std::path::PathBuf {
    let lexical = std::path::PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../..");
    // A missing root is left lexical: the fixture manifests this root locates
    // report that absence themselves, with the path they tried.
    std::fs::canonicalize(&lexical).unwrap_or(lexical)
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
///
/// # Errors
///
/// Returns the error when a workspace Cargo configuration path exists but
/// cannot be inspected, so an unreadable configuration refuses the
/// preparation instead of silently leaving it out of the input hash.
fn workspace_inputs() -> std::io::Result<Vec<std::path::PathBuf>> {
    let workspace = workspace_root();
    let mut roots: Vec<std::path::PathBuf> = ["Cargo.toml", "Cargo.lock", "crates"]
        .into_iter()
        .map(|entry| workspace.join(entry))
        .collect();
    roots.extend(nested_cargo::cargo_config_inputs(&workspace)?);
    Ok(roots)
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

/// One fixture's preparation family.
///
/// Both fixtures make up a `cargo test --no-run --locked --offline` closure
/// over the same workspace inputs, so everything that differs between them
/// travels together here: the family name, the artefact subdirectory, the
/// fixture source tree, and the package the family speaks for. The
/// environment view and the variant label are identical for both, so
/// [`fixture_preparation`] supplies them rather than each caller.
///
/// `package` names the fixture the family builds, and is what
/// [`clean_fixture_units`](super::clean_fixture_units) later removes. It is
/// *not* part of the ownership claim — see [`owned_packages`].
#[derive(Clone, Copy)]
struct Family {
    /// Name the family's stamp is filed under.
    name: &'static str,
    /// Subdirectory of `<target>/tests` the family's artefacts live in.
    artefact_subdir: &'static str,
    /// The fixture source tree this family hashes and builds.
    source: fn() -> std::path::PathBuf,
    /// The scratch copy the family's nested builds actually run in.
    ///
    /// Kept apart from [`Family::source`]: that is the checked-in tree that is
    /// hashed, while this is the copy Cargo runs in, so it is the directory a
    /// relative `CARGO_HOME` or `RUSTC` resolves against.
    scratch: fn() -> std::path::PathBuf,
    /// Cargo package the fixture's build produces and its experiment cleans.
    package: &'static str,
}

/// The scratch rebuild-invalidation fixture.
const REBUILD: Family = Family {
    name: REBUILD_FAMILY,
    artefact_subdir: "rebuild-invalidation",
    source: fixtures::source_fixture_dir,
    scratch: fixtures::scratch_fixture_dir,
    package: REBUILD_PACKAGE,
};

/// The scratch build-script addition fixture.
const ADDITION: Family = Family {
    name: ADDITION_FAMILY,
    artefact_subdir: "feature-addition",
    source: fixtures::source_addition_dir,
    scratch: fixtures::scratch_addition_dir,
    package: ADDITION_PACKAGE,
};

/// The packages a family's stamp vouches for.
///
/// The fixture itself is deliberately excluded. Both experiments run
/// [`clean_fixture_units`](super::clean_fixture_units) — a `cargo clean -p
/// <fixture>` — before their baseline by design, which removes that package's
/// `.d` records and its build-script `out/` directories. A stamp that recorded
/// them would be refused on the very next assertion, because the experiment
/// destroys them on purpose: the rebuild of those units is the contract under
/// test, so they can never be "prepared". The preparation's job is the
/// *dependency* graph, which the clean leaves alone.
///
/// The workspace crates are declared because they are what the preparation
/// actually warms. Both fixtures declare path dependencies on `rstest-bdd` and
/// `rstest-bdd-macros`, and each pulls the three harness-side crates in turn,
/// so `PREPARE_ARGS` compiles all five into the shared profile. Leaving them
/// out would filter their records from both the recorded and the present
/// lists, and an evicted dependency would then compare equal to itself and
/// pass the gate.
fn owned_packages(_family: Family) -> Vec<String> {
    WORKSPACE_DEPENDENCIES
        .iter()
        .copied()
        .map(str::to_owned)
        .collect()
}

/// Describe `family`'s preparation.
///
/// # Errors
///
/// Returns the I/O error from hashing the workspace inputs or from resolving
/// the child environment's view.
fn fixture_preparation(
    env: &process::ChildEnv,
    family: Family,
) -> std::io::Result<nested_cargo::Preparation> {
    let mut roots = vec![(family.source)()];
    roots.extend(workspace_inputs()?);
    // The scratch copy is where the nested builds actually run, and Cargo's
    // configuration walk starts there rather than at the workspace. A
    // configuration between the two — or one in the scratch copy itself — sets
    // profile options or a linker the build reads and the workspace inputs
    // never reach. Missing it let a stamp survive a configuration-only edit and
    // left dependency recompilation inside the timed test, which is the same
    // gap `workspace_inputs` closes for the workspace's own files.
    //
    // The copy may not exist yet on a cold run: this describes the preparation
    // before the callback creates it. An absent tree cannot hold configuration,
    // and the workspace inputs above already cover what Cargo reads from there,
    // so the walk contributes nothing rather than refusing the preparation.
    let scratch = (family.scratch)();
    if scratch.is_dir() {
        roots.extend(nested_cargo::cargo_config_inputs(&scratch)?);
    }
    let inputs = nested_cargo::input_hash(&roots)?;
    let mut preparation = nested_cargo::Preparation::new(
        nested_cargo::Sources::new(family.name, "locked-offline-test-no-run", inputs),
        nested_cargo::Directories::new(
            artefact_directory(process::child_target_directory(env), family.artefact_subdir),
            // The scratch fixture each nested build runs in, not the artefact
            // directory it stamps: a relative `RUSTC` or `CARGO_HOME` resolves
            // against the command's directory, and these builds run in the copy.
            (family.scratch)(),
        ),
        process::preparation_view(env)?,
    )?;
    preparation.owned_packages = owned_packages(family);
    Ok(preparation)
}

/// Describe the rebuild fixture's preparation family.
fn rebuild_preparation(env: &process::ChildEnv) -> std::io::Result<nested_cargo::Preparation> {
    fixture_preparation(env, REBUILD)
}

/// Describe the addition fixture's preparation family.
fn addition_preparation(env: &process::ChildEnv) -> std::io::Result<nested_cargo::Preparation> {
    fixture_preparation(env, ADDITION)
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
mod tests;
