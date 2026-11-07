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

#[test]
fn a_relative_target_directory_stamps_where_the_child_builds() {
    // The nested build runs in the scratch fixture directory, not in this
    // process's directory, so Cargo would resolve a relative
    // `CARGO_TARGET_DIR` to a different place than the stamp did. The
    // harness must resolve it once, before storing it, so both name the
    // same tree and evicting that tree also evicts the stamp.
    let env = child_env(std::path::Path::new("relative-cache"));
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

    let preparation = fixture_preparation(
        &env,
        REBUILD_FAMILY,
        "rebuild-invalidation",
        fixtures::source_fixture_dir(),
    )
    .expect("preparation description");
    assert!(
        preparation.directory.starts_with(resolved),
        "the stamp must be under the resolved directory: {}",
        preparation.directory.display()
    );
}

#[test]
fn evicting_only_the_profile_invalidates_the_preparation() {
    // `clean_fixture_units` runs `cargo clean -p <fixture>`, and a plain
    // `rm -rf <target>/debug` does the same thing more broadly. Both leave
    // the family's scratch root and its stamp untouched, because the
    // profile directory is a sibling of it. The gate must still refuse:
    // the compiled units the timed assertion reuses are gone.
    let external = tempfile::tempdir().expect("external target dir");
    let env = child_env(external.path());
    let preparation = fixture_preparation(
        &env,
        REBUILD_FAMILY,
        "rebuild-invalidation",
        fixtures::source_fixture_dir(),
    )
    .expect("preparation description");

    let profile = process::child_target_directory(&env).join(nested_cargo::DEFAULT_PROFILE);
    std::fs::create_dir_all(&profile).expect("profile dir");

    nested_cargo::prepare_now(&preparation, || {
        Ok(nested_cargo::PreparationSummary::default())
    })
    .expect("preparation");
    nested_cargo::ensure_prepared_with(&preparation, true, || {
        panic!("a just-prepared family must satisfy the gate")
    })
    .expect("the freshly written stamp must be accepted");

    std::fs::remove_dir_all(&profile).expect("evict only the profile");
    assert!(
        preparation.directory.is_dir(),
        "the family's scratch root and its stamp must survive: that is why the stamp alone cannot \
         answer this question"
    );

    let error = nested_cargo::ensure_prepared_with(&preparation, true, || {
        panic!("a timed test must never prepare")
    })
    .expect_err("an evicted profile must fail the gate");
    assert_eq!(error.kind(), std::io::ErrorKind::NotFound);
}
