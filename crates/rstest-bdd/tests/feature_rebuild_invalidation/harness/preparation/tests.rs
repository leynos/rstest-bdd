//! Regression coverage for where the preparation stamp lives.
//!
//! Exercises this family's `fixture_preparation` and `workspace_inputs`
//! helpers together with the harness's `prepare_now`, `ensure_prepared_building`
//! and `input_hash`, over the child environment the parent module resolves. A
//! stamp is only meaningful if it is evicted together with the artefacts it
//! vouches for. An inherited `CARGO_TARGET_DIR` moves those artefacts out of
//! the workspace `target/`, so a stamp stored in the scratch root would survive
//! an eviction of the external directory, keep matching, and let the timed
//! assertion pay for a cold dependency compile — the exact failure issue 910
//! removes.
//!
//! The cases below are the claims about the gate; the setup they share lives in
//! the sibling [`prepared`] module. A sibling `ownership` module covers the
//! family's declaration of which packages its build produced.

use rstest_bdd_harness::nested_cargo;

use super::{REBUILD, fixture_preparation, workspace_inputs};
use crate::harness::{fixtures, process};

// The families' ownership declaration, exercised against what their fixtures
// actually resolve to. Its own module because it is a separate claim from the
// stamp-location regressions here.
#[path = "tests/ownership.rs"]
mod ownership;

// The setup the eviction cases share: an external target directory, a family
// described against it, a populated profile, and a stamp written and accepted.
// Its own module because it is several times the size of any case that uses it.
#[path = "tests/prepared.rs"]
mod prepared;

use prepared::{child_env, evict, prepared_family, scratch_fixture, stamp_file};

#[test]
fn an_inherited_target_directory_moves_the_stamp_with_the_artefacts() {
    let external = tempfile::tempdir().expect("external target dir");
    let env = child_env(external.path()).expect("the fixture environment should resolve");
    assert_eq!(
        process::child_target_directory(&env),
        external.path(),
        "an inherited CARGO_TARGET_DIR must win over the workspace default"
    );

    let preparation = fixture_preparation(&env, REBUILD).expect("preparation description");

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
    let env =
        process::child_env_from_vars(Vec::new()).expect("the fixture environment should resolve");
    assert_eq!(
        process::child_target_directory(&env),
        fixtures::shared_target_dir()
    );
}

#[test]
fn a_configuration_only_change_invalidates_the_preparation() {
    // Cargo discovers configuration by walking up from the nested build's
    // working directory, so the committed workspace `.cargo/config.toml`
    // applies to both fixtures here. It sets the development-profile options
    // and the Linux linker flags: editing either rebuilds dependency artefacts
    // without touching a fixture source, so a stamp that ignored it would
    // survive the edit and the timed assertion would compile the graph.
    let workspace = super::workspace_root();
    let workspace_config = workspace.join(".cargo/config.toml");
    assert!(
        workspace_config.is_file(),
        "this regression is about the configuration this repository actually commits; {} is \
         missing",
        workspace_config.display()
    );

    let roots = workspace_inputs().expect("workspace inputs");
    // `workspace_root` resolves and `cargo_config_inputs` reports through that
    // resolved form, so the configuration arrives as `.cargo/config.toml`
    // beneath the same resolved root rather than as this lexical join. Asking
    // by containment states the question the case means — is this file among
    // the inputs — and holds whichever form the probe happens to use, which is
    // what the plain equality here missed on the Windows lane.
    let cargo_dir = workspace.join(".cargo");
    let is_configuration = |root: &std::path::PathBuf| root.starts_with(&cargo_dir);
    assert!(
        roots.iter().any(is_configuration),
        "the configuration must be among the family's own inputs: {roots:?}"
    );

    let with = nested_cargo::input_hash(&roots).expect("hash with the configuration");
    let without: Vec<std::path::PathBuf> = roots
        .iter()
        .filter(|root| !is_configuration(root))
        .cloned()
        .collect();
    let without = nested_cargo::input_hash(&without).expect("hash without the configuration");
    assert_ne!(
        with, without,
        "a configuration-only change must invalidate the prepared artefacts"
    );
}

#[rstest::rstest]
fn evicting_the_artefacts_invalidates_the_preparation(
    #[from(prepared_family)] prepared: std::io::Result<prepared::PreparedFamily>,
) {
    // Prepare a family whose artefacts live outside the scratch tree, then
    // evict exactly that directory while leaving the scratch fixture alone.
    // The gate must refuse: the artefacts the stamp vouched for are gone,
    // and a timed assertion that trusted the stamp would rebuild the
    // dependency graph inside its own deadline.
    //
    // The fixture populated the profile before writing the stamp, so a
    // refusal at the end cannot come from artefacts that never existed.
    let prepared = prepared.expect("the prepared family fixture");
    let preparation = &prepared.preparation;
    let scratch = scratch_fixture().expect("scratch fixture");
    assert!(scratch.path().is_dir(), "the scratch fixture must survive");
    // Guard before evicting: a regression that stamped the shared scratch
    // root would otherwise leave a stamp in a directory this test does not
    // own.
    assert!(
        preparation
            .directory
            .starts_with(process::child_target_directory(&prepared.env)),
        "the stamp must live inside the resolved target directory: {}",
        preparation.directory.display()
    );

    evict(process::child_target_directory(&prepared.env)).expect("evict the artefact directory");

    let error = nested_cargo::ensure_prepared_building(preparation, true, || {
        panic!("a timed test must never prepare")
    })
    .expect_err("the evicted artefacts must fail the gate");
    assert_eq!(error.kind(), std::io::ErrorKind::NotFound);
    assert!(
        scratch.path().is_dir(),
        "the scratch fixture must be untouched"
    );
}

#[test]
fn a_relative_target_directory_stamps_where_the_child_builds() {
    // The nested build runs in the scratch fixture directory, not in this
    // process's directory, so Cargo would resolve a relative
    // `CARGO_TARGET_DIR` to a different place than the stamp did. The
    // harness must resolve it once, before storing it, so both name the
    // same tree and evicting that tree also evicts the stamp.
    let env = child_env(std::path::Path::new("relative-cache"))
        .expect("the fixture environment should resolve");
    let resolved = process::child_target_directory(&env);
    assert!(
        resolved.is_absolute(),
        "the stored target directory must be absolute: {}",
        resolved.display()
    );
    assert!(
        resolved.ends_with("relative-cache"),
        "the inherited value must be preserved as the final component: {}",
        resolved.display()
    );

    let preparation = fixture_preparation(&env, REBUILD).expect("preparation description");
    assert!(
        preparation.directory.starts_with(resolved),
        "the stamp must be under the resolved directory: {}",
        preparation.directory.display()
    );
}

#[rstest::rstest]
fn evicting_only_the_profile_invalidates_the_preparation(
    #[from(prepared_family)] prepared: std::io::Result<prepared::PreparedFamily>,
) {
    // `clean_fixture_units` runs `cargo clean -p <fixture>`, and a plain
    // `rm -rf <target>/debug` does the same thing more broadly. Both leave
    // the family's scratch root and its stamp untouched, because the
    // profile directory is a sibling of it. The gate must still refuse:
    // the compiled units the timed assertion reuses are gone.
    let prepared = prepared.expect("the prepared family fixture");
    let preparation = &prepared.preparation;

    evict(&prepared.profile).expect("evict only the profile");
    // The stamp *file*, not merely the directory holding it. A refusal caused
    // by a missing stamp would say nothing about artefacts: the gate would be
    // reporting that nothing was ever prepared. Asserting the file is present
    // immediately before the call is what makes the refusal evidence that the
    // recorded artefact names were consulted and found gone.
    assert!(
        stamp_file(preparation).is_file(),
        "the family's stamp must survive the profile eviction: {}",
        stamp_file(preparation).display()
    );

    let error = nested_cargo::ensure_prepared_building(preparation, true, || {
        panic!("a timed test must never prepare")
    })
    .expect_err("an evicted profile must fail the gate");
    assert_eq!(error.kind(), std::io::ErrorKind::NotFound);
}

#[rstest::rstest]
fn evicting_the_compiled_units_invalidates_the_preparation(
    #[from(prepared_family)] prepared: std::io::Result<prepared::PreparedFamily>,
) {
    // The profile directory can survive while the work inside it does not.
    // This is the partial eviction a gate that only checked `is_dir()` would
    // accept: `<target>/debug` still exists, so does the family's stamp, and
    // Cargo must still rebuild every dependency unit.
    let prepared = prepared.expect("the prepared family fixture");
    let preparation = &prepared.preparation;

    // Remove exactly one of the two directories Cargo needs, then require the
    // gate to refuse. Removing both would also exercise it, but only this
    // shape proves that either one alone is enough to invalidate a stamp.
    evict(&prepared.profile.join("deps")).expect("evict the compiled units");
    assert!(
        prepared.profile.is_dir(),
        "the profile directory must survive"
    );
    // As above: the parent directory surviving proves only that the profile
    // was partly evicted. The stamp is the thing being contradicted, so the
    // gate can only be said to have refused *because of* the missing units if
    // the stamp it read was there.
    assert!(
        stamp_file(preparation).is_file(),
        "the family's stamp must survive the unit eviction: {}",
        stamp_file(preparation).display()
    );

    let error = nested_cargo::ensure_prepared_building(preparation, true, || {
        panic!("a timed test must never prepare")
    })
    .expect_err("an evicted dependency graph must fail the gate");
    assert_eq!(error.kind(), std::io::ErrorKind::NotFound);
    let message = error.to_string();
    assert!(
        message.contains("artefacts it vouched for are gone"),
        "the message must name the evicted artefacts: {message}"
    );
}
