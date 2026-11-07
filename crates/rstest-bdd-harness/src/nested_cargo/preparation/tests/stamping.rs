//! The preparation lifecycle: stamping, the gate that reads it, and eviction.

use std::{fs, io};

use super::{
    super::stamp::stamp_path,
    environment_in,
    fixture_package,
    populate_profile,
    preparation,
    scratch_preparation,
};
use crate::nested_cargo::{
    DEFAULT_PROFILE,
    DEPENDENCY_ARTEFACT_DIRS,
    Preparation,
    PreparationSummary,
    ProfileDestination,
    ensure_prepared_with,
    env_from_vars,
    input_hash,
    prepare_now,
};

#[test]
fn lazy_preparation_writes_the_stamp_and_is_idempotent() {
    let (_directory, preparation, _environment) = scratch_preparation();
    let mut runs = 0_u32;

    for _ in 0..2 {
        ensure_prepared_with(&preparation, false, || {
            runs += 1;
            Ok(PreparationSummary { compiled_units: 3 })
        })
        .expect("lazy preparation should succeed");
    }

    assert_eq!(runs, 1, "the second call should see the stamp");
    assert!(stamp_path(&preparation).is_file());
}

#[test]
fn prepare_now_runs_under_nextest_and_stamps_once() {
    let (_directory, preparation, _environment) = scratch_preparation();
    let mut runs = 0_u32;
    for _ in 0..2 {
        prepare_now(&preparation, || {
            runs += 1;
            Ok(PreparationSummary { compiled_units: 1 })
        })
        .expect("explicit preparation should succeed");
    }
    assert_eq!(runs, 1, "a warm family should not prepare twice");
}

#[test]
fn preparation_survives_the_build_output_it_creates() {
    // The stamp is computed from the inputs before preparation runs, so
    // anything that preparation itself writes inside an input root must not
    // contribute to the hash. If it did, a cold preparation would write a
    // stamp that was already stale, and the timed test that follows would
    // fail its gate every time.
    let (directory, source) = fixture_package();

    // A real target directory: the gate now also requires the profile
    // directory the compiled units land in, and the dependency artefacts
    // inside it, so a synthetic path would refuse the stamp for a reason this
    // test is not about.
    let target = directory.path().join("target");
    populate_profile(&target);
    let environment = environment_in(&target);
    let prepare = || {
        // What `trybuild` and a nested Cargo leave behind: its WIP scratch
        // beside the crate, and a target directory for the build output.
        fs::create_dir_all(source.join("wip")).expect("wip dir");
        fs::write(source.join("wip/.gitignore"), "*\n").expect("wip scratch");
        fs::create_dir_all(source.join("target/debug")).expect("target dir");
        fs::write(source.join("target/debug/artifact"), "binary").expect("artefact");
        Ok(PreparationSummary::default())
    };

    let inputs = input_hash(std::slice::from_ref(&source)).expect("inputs before preparation");
    let family = preparation(&source, &environment, "v", &inputs);
    prepare_now(&family, prepare).expect("preparation");

    // A second run recomputes the hash over the now-populated tree; it must
    // still match, or the first timed test would be refused its gate.
    let after = input_hash(std::slice::from_ref(&source)).expect("inputs after preparation");
    let reopened = preparation(&source, &environment, "v", &after);
    let mut runs = 0_u32;
    ensure_prepared_with(&reopened, true, || {
        runs += 1;
        Ok(PreparationSummary::default())
    })
    .expect("a just-prepared family must satisfy the nextest gate");
    assert_eq!(runs, 0, "the gate must not need to prepare again");
}

#[test]
fn failed_preparation_leaves_no_stamp() {
    let (_directory, preparation, _environment) = scratch_preparation();
    let error = ensure_prepared_with(&preparation, false, || {
        Err(io::Error::other("nested build failed"))
    })
    .expect_err("preparation should fail");
    assert_eq!(error.kind(), io::ErrorKind::Other);
    assert!(!stamp_path(&preparation).exists());
}

#[test]
fn nextest_refuses_to_prepare_and_names_the_command() {
    let (_directory, preparation, _environment) = scratch_preparation();
    let error = ensure_prepared_with(&preparation, true, || {
        panic!("nextest must not run preparation in a timed test")
    })
    .expect_err("a missing stamp under nextest must fail");
    assert_eq!(error.kind(), io::ErrorKind::NotFound);
    let message = error.to_string();
    assert!(
        message.contains("make prepare-nested-fixtures"),
        "{message}"
    );
    assert!(message.contains("nested_fixture_preparation"), "{message}");
}

#[test]
fn a_relative_target_directory_is_resolved_once_before_it_reaches_the_child() {
    // Cargo resolves a relative `CARGO_TARGET_DIR` against the working
    // directory of the command that reads it. Every nested build here runs in
    // the scratch fixture directory rather than in this process's directory,
    // so an unresolved relative value would name one place for the stamp and
    // another for the artefacts — and evicting the artefacts would leave a
    // matching stamp behind.
    let environment = env_from_vars(
        vec![("CARGO_TARGET_DIR".into(), "relative-cache".into())],
        None,
        ProfileDestination::Caller,
    );
    let cwd = std::env::current_dir().expect("current dir");

    assert_eq!(
        environment.get("CARGO_TARGET_DIR"),
        Some(cwd.join("relative-cache").as_os_str()),
        "the child must receive an absolute path, because it does not run in this directory"
    );
    assert_eq!(
        environment.target_directory().as_deref(),
        Some(cwd.join("relative-cache").as_path()),
        "the resolved accessor must agree with the value the child receives"
    );
}

#[test]
fn evicting_only_the_profile_invalidates_the_preparation() {
    // A stamp lives in the family's own scratch root, which is a *sibling* of
    // Cargo's profile directory rather than a descendant: evicting
    // `<target>/debug` removes every compiled dependency unit while leaving
    // the stamp and its scratch root intact. A gate that consulted only the
    // stamp would admit a timed assertion against an empty profile, and that
    // assertion would rebuild the whole graph inside its own deadline.
    let (_directory, preparation, _environment) = scratch_preparation();
    let profile = preparation
        .artefacts
        .first()
        .expect("the preparation describes the profile directory it reuses")
        .clone();

    prepare_now(&preparation, || Ok(PreparationSummary::default())).expect("preparation");
    ensure_prepared_with(&preparation, true, || {
        panic!("a just-prepared family must satisfy the gate")
    })
    .expect("the freshly written stamp must be accepted");

    fs::remove_dir_all(&profile).expect("evict the profile");
    assert!(
        stamp_path(&preparation).is_file(),
        "the stamp must survive an eviction that removes only the profile: that is exactly why \
         the stamp alone cannot answer this question"
    );

    let error = ensure_prepared_with(&preparation, true, || {
        panic!("a timed test must never prepare")
    })
    .expect_err("an evicted profile must fail the gate");
    assert_eq!(error.kind(), io::ErrorKind::NotFound);
    let message = error.to_string();
    assert!(
        message.contains("artefacts it vouched for are gone"),
        "the message must name the evicted artefacts: {message}"
    );
    assert!(message.contains("debug"), "{message}");
}

#[test]
fn evicting_the_dependency_artefacts_invalidates_the_preparation() {
    // The profile directory can survive while the work inside it does not.
    // Removing `<profile>/deps` leaves `<profile>`, the family's stamp root and
    // the stamp all in place, and Cargo must still rebuild every dependency
    // unit the timed assertion would link against. A gate that required only
    // the profile directory would admit that cold build.
    let (_directory, preparation, _environment) = scratch_preparation();
    let profile = preparation
        .artefacts
        .first()
        .expect("the preparation describes the profile directory it reuses")
        .clone();

    prepare_now(&preparation, || Ok(PreparationSummary::default())).expect("preparation");
    ensure_prepared_with(&preparation, true, || {
        panic!("a just-prepared family must satisfy the gate")
    })
    .expect("the freshly written stamp must be accepted");

    let deps = profile.join("deps");
    fs::remove_dir_all(&deps).expect("evict the dependency artefacts");
    assert!(profile.is_dir(), "the profile directory must survive");
    assert!(stamp_path(&preparation).is_file(), "the stamp must survive");

    let error = ensure_prepared_with(&preparation, true, || {
        panic!("a timed test must never prepare")
    })
    .expect_err("an evicted dependency graph must fail the gate");
    assert_eq!(error.kind(), io::ErrorKind::NotFound);
    let message = error.to_string();
    assert!(
        message.contains("artefacts it vouched for are gone"),
        "the message must name the evicted artefacts: {message}"
    );
    assert!(message.contains("debug"), "{message}");
}

#[test]
fn an_empty_profile_directory_is_not_a_prepared_family() {
    // `mkdir -p <target>/debug` is not a build. The gate must not read a bare
    // directory as evidence that the dependency graph exists.
    let (directory, preparation, _environment) = scratch_preparation();
    let profile = preparation
        .artefacts
        .first()
        .expect("the preparation describes the profile directory it reuses")
        .clone();
    fs::remove_dir_all(&profile).expect("evict the populated profile");
    fs::create_dir_all(&profile).expect("recreate an empty profile");

    prepare_now(&preparation, || Ok(PreparationSummary::default())).expect("preparation");
    // `prepare_now` refuses too: its re-check uses the same predicate, so an
    // empty profile forces the preparation rather than stamping over it.
    let error = ensure_prepared_with(&preparation, true, || {
        panic!("a timed test must never prepare")
    })
    .expect_err("an empty profile must fail the gate");
    assert_eq!(error.kind(), io::ErrorKind::NotFound);
    assert!(
        DEPENDENCY_ARTEFACT_DIRS
            .iter()
            .all(|name| !profile.join(name).exists()),
        "the fixture must leave the profile genuinely empty: {}",
        directory.path().display()
    );
}

#[test]
fn for_fixture_vouches_for_the_profile_its_own_builds_populate() {
    // The trybuild and UI-lint families set the child's `CARGO_TARGET_DIR` to
    // exactly the directory they stamp, so their profile directory is a
    // *descendant* of the stamp root rather than the sibling the scratch
    // families have. The gate must still name it: emptying a profile without
    // removing the family's stamp is possible there too (`cargo clean -p`, or
    // a targeted `rm -rf <root>/debug`), and the dependency graph it holds is
    // the work a timed assertion would otherwise redo.
    let directory = tempfile::tempdir().expect("fixture dir");
    let preparation =
        Preparation::for_fixture("unit-fixture", directory.path().to_path_buf(), "v", "hash");
    assert_eq!(
        preparation.artefacts,
        vec![directory.path().join(DEFAULT_PROFILE)],
        "a fixture family must vouch for the profile its own nested builds write"
    );
}

#[test]
fn stale_fingerprint_forces_preparation() {
    let (directory, first, environment) = scratch_preparation();
    ensure_prepared_with(&first, false, || Ok(PreparationSummary::default()))
        .expect("initial preparation");

    let changed = preparation(directory.path(), &environment, "v", "changed-hash");
    let error = ensure_prepared_with(&changed, true, || {
        panic!("a stale stamp must not count as prepared")
    })
    .expect_err("stale inputs must fail under nextest");
    assert_eq!(error.kind(), io::ErrorKind::NotFound);
}
