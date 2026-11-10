//! The preparation lifecycle: stamping and the gate that reads it.
//!
//! Eviction — the states a stamp cannot see, where the profile beneath it has
//! been removed or gutted — lives in `eviction`, so this module covers only
//! writing a stamp and consulting one.

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

/// Write a stamp that exists but cannot be read as text.
///
/// Invalid UTF-8 makes `read_to_string` fail with `InvalidData`, which is the
/// same "present but unreadable" state a permission bit would produce, without
/// depending on the permission model — a suite running as root can read a file
/// whose mode is zero. The file is left in place rather than made a directory
/// so a later preparation can replace it, which is what the `cargo test` case
/// below needs to reach its successful end.
fn write_unreadable_stamp(preparation: &Preparation) {
    fs::write(stamp_path(preparation), [0xff, 0xfe]).expect("unreadable stamp");
}

#[test]
fn nextest_reports_an_unreadable_stamp_rather_than_calling_it_missing() {
    // The two runners need opposite answers from an unreadable stamp, so the
    // branch that fails fast exists only at this boundary: the query in
    // `stamp` returns the error and decides nothing. Without this test the
    // nextest branch is unexercised, and it would be possible to report
    // "missing or stale" for a tree that is present but cannot be vouched
    // for — which sends the reader looking for an evicted directory.
    let (_directory, preparation, _environment) = scratch_preparation();
    write_unreadable_stamp(&preparation);

    let error = ensure_prepared_with(&preparation, true, || {
        panic!("an unreadable stamp under nextest must not prepare")
    })
    .expect_err("an unreadable stamp must refuse the gate");
    let message = error.to_string();
    assert!(message.contains("exists but cannot be read"), "{message}");
    assert!(
        message.contains("make prepare-nested-fixtures"),
        "the message must still name the remedy: {message}"
    );
    assert!(
        !message.contains("missing or stale"),
        "an unreadable stamp must not be reported as a missing one: {message}"
    );
}

#[test]
fn cargo_test_prepares_again_behind_an_unreadable_stamp() {
    // The complement of the case above: under `cargo test` the same fact only
    // costs a rebuild, so the boundary must choose to prepare rather than
    // fail. Running the closure is what proves the branch was taken — had the
    // boundary failed fast, it would not have run.
    let (_directory, preparation, _environment) = scratch_preparation();
    write_unreadable_stamp(&preparation);
    let mut runs = 0_u32;

    ensure_prepared_with(&preparation, false, || {
        runs += 1;
        Ok(PreparationSummary { compiled_units: 1 })
    })
    .expect("cargo test must recover from an unreadable stamp by preparing");

    assert_eq!(
        runs, 1,
        "under cargo test an unreadable stamp must lead to preparation, not a refusal"
    );
    // The replacement stamp is readable, so the next gate reuses it rather
    // than paying for the build again. An unreadable stamp that survived the
    // preparation would cost a rebuild on every single run.
    let mut runs_after = runs;
    ensure_prepared_with(&preparation, false, || {
        runs_after += 1;
        Ok(PreparationSummary::default())
    })
    .expect("the stamp written by the recovery must be reusable");
    assert_eq!(runs_after, runs, "the recovered stamp must be honoured");
}

#[test]
fn a_family_prepared_by_the_preparation_test_satisfies_the_nextest_gate() {
    // The whole point of the split is that two different test processes reach
    // the same verdict from the same directory: the preparation test builds,
    // and every timed test afterwards only reads. Each half is covered above
    // in isolation; this runs the sequence, because the halves agree only if
    // the stamp the preparation wrote is the stamp the gate then accepts. A
    // mismatch would show up as every timed test paying for a cold build —
    // exactly the failure the issue is about.
    let (directory, family, environment) = scratch_preparation();
    let mut builds = 0_u32;

    // First: the timed test, which must refuse rather than build.
    let refused = ensure_prepared_with(&family, true, || {
        panic!("a timed test must not prepare its own family")
    })
    .expect_err("an unprepared family must fail the nextest gate");
    assert_eq!(refused.kind(), io::ErrorKind::NotFound);

    // Second: the named preparation test, run under nextest, which builds.
    prepare_now(&family, || {
        builds += 1;
        Ok(PreparationSummary { compiled_units: 2 })
    })
    .expect("the preparation test should build the family");
    assert_eq!(builds, 1);

    // Third: the timed test again, now over a freshly stamped family. It is
    // reopened from the same description so the check does not lean on any
    // in-process state the failed attempt left behind.
    let reopened = preparation(directory.path(), &environment, "v", "hash");
    ensure_prepared_with(&reopened, true, || {
        panic!("the gate must accept the family the preparation test stamped")
    })
    .expect("a family prepared by the preparation test must pass the nextest gate");
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
