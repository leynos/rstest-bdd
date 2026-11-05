//! Behavioural coverage for the stamp protocol and runner policy.

use std::{fs, io, path::Path};

use super::{
    Preparation,
    PreparationSummary,
    count_compiling,
    ensure_prepared_with,
    input_hash,
    prepare_now,
    stamp::{PREPARED_FILE_SUFFIX, fingerprint, stamp_path},
};
use crate::nested_cargo::{NestedCargoEnv, ProfileDestination, env_from_vars};

/// A filtered environment carrying only fingerprint-relevant variables.
fn environment() -> NestedCargoEnv {
    env_from_vars(
        vec![
            ("RUSTFLAGS".into(), "-Dwarnings".into()),
            ("NEXTEST_RUN_ID".into(), "run-1".into()),
            ("LLVM_PROFILE_FILE".into(), "/tmp/parent.profraw".into()),
            ("PATH".into(), "/usr/bin".into()),
        ],
        Some(Path::new("/tmp/target")),
        ProfileDestination::Caller,
    )
}

/// A preparation rooted at `directory`.
fn preparation(
    directory: &Path,
    environment: &NestedCargoEnv,
    variant: &str,
    inputs: &str,
) -> Preparation {
    Preparation::new(
        "unit",
        directory.to_path_buf(),
        environment.clone(),
        variant,
        inputs,
    )
}

#[test]
fn fingerprint_excludes_runner_and_coverage_variables() {
    let environment = environment();
    let text = fingerprint(&preparation(
        Path::new("/tmp/target"),
        &environment,
        "all-features",
        "abc",
    ));
    assert!(text.contains("RUSTFLAGS=-Dwarnings"));
    assert!(text.contains("CARGO_TARGET_DIR=/tmp/target"));
    assert!(text.contains("variant: all-features"));
    assert!(text.contains("inputs: abc"));
    assert!(!text.contains("NEXTEST_RUN_ID"));
    assert!(!text.contains("LLVM_PROFILE_FILE"));
    assert!(!text.contains("PATH="));
}

#[test]
fn fingerprint_changes_with_variant_and_inputs() {
    let environment = environment();
    let one = fingerprint(&preparation(
        Path::new("/tmp/target"),
        &environment,
        "all-features",
        "abc",
    ));
    let other = fingerprint(&preparation(
        Path::new("/tmp/target"),
        &environment,
        "no-default-features",
        "abc",
    ));
    let third = fingerprint(&preparation(
        Path::new("/tmp/target"),
        &environment,
        "all-features",
        "def",
    ));
    assert_ne!(one, other);
    assert_ne!(one, third);
}

#[test]
fn lazy_preparation_writes_the_stamp_and_is_idempotent() {
    let directory = tempfile::tempdir().expect("temp dir");
    let environment = environment();
    let preparation = preparation(directory.path(), &environment, "v", "hash");
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
    let directory = tempfile::tempdir().expect("temp dir");
    let environment = environment();
    let preparation = preparation(directory.path(), &environment, "v", "hash");
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
    let directory = tempfile::tempdir().expect("temp dir");
    let source = directory.path().join("fixture");
    fs::create_dir_all(&source).expect("fixture dir");
    fs::write(source.join("Cargo.toml"), "[package]\nname = \"f\"\n").expect("manifest");

    let environment = environment();
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
    let directory = tempfile::tempdir().expect("temp dir");
    let environment = environment();
    let preparation = preparation(directory.path(), &environment, "v", "hash");
    let error = ensure_prepared_with(&preparation, false, || {
        Err(io::Error::other("nested build failed"))
    })
    .expect_err("preparation should fail");
    assert_eq!(error.kind(), io::ErrorKind::Other);
    assert!(!stamp_path(&preparation).exists());
}

#[test]
fn nextest_refuses_to_prepare_and_names_the_command() {
    let directory = tempfile::tempdir().expect("temp dir");
    let environment = environment();
    let preparation = preparation(directory.path(), &environment, "v", "hash");
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
fn stale_fingerprint_forces_preparation() {
    let directory = tempfile::tempdir().expect("temp dir");
    let environment = environment();
    let first = preparation(directory.path(), &environment, "v", "hash");
    ensure_prepared_with(&first, false, || Ok(PreparationSummary::default()))
        .expect("initial preparation");

    let changed = preparation(directory.path(), &environment, "v", "changed-hash");
    let error = ensure_prepared_with(&changed, true, || {
        panic!("a stale stamp must not count as prepared")
    })
    .expect_err("stale inputs must fail under nextest");
    assert_eq!(error.kind(), io::ErrorKind::NotFound);
}

#[test]
fn input_hash_ignores_stamps_and_build_output() {
    let directory = tempfile::tempdir().expect("temp dir");
    let source = directory.path().join("fixture");
    fs::create_dir_all(source.join("features")).expect("fixture dir");
    fs::write(source.join("Cargo.toml"), "[package]\nname = \"f\"\n").expect("manifest");
    fs::write(source.join("features/a.feature"), "Feature: a\n").expect("feature");
    let before = input_hash(std::slice::from_ref(&source)).expect("initial hash");

    fs::create_dir_all(source.join("target/debug")).expect("target dir");
    fs::write(source.join("target/debug/artifact"), "binary").expect("artefact");
    fs::write(
        source.join(format!(".unit{PREPARED_FILE_SUFFIX}")),
        "different fingerprint",
    )
    .expect("stamp");
    let after = input_hash(std::slice::from_ref(&source)).expect("hash after build");
    assert_eq!(before, after, "build output must not change the inputs");

    fs::write(source.join("features/a.feature"), "Feature: changed\n").expect("edit");
    let changed = input_hash(&[source]).expect("hash after edit");
    assert_ne!(before, changed, "fixture edits must change the inputs");
}

#[test]
fn input_hash_ignores_trybuild_wip_scratch() {
    // `trybuild` writes `wip/` beside the crate it tests, not inside the
    // target directory, and it creates that directory during the very build a
    // preparation stamps. Hashing it would invalidate every stamp the moment
    // it was written, because the hash is taken before that build runs.
    let directory = tempfile::tempdir().expect("temp dir");
    let source = directory.path().join("fixture");
    fs::create_dir_all(&source).expect("fixture dir");
    fs::write(source.join("Cargo.toml"), "[package]\nname = \"f\"\n").expect("manifest");
    let before = input_hash(std::slice::from_ref(&source)).expect("initial hash");

    fs::create_dir_all(source.join("wip")).expect("wip dir");
    fs::write(source.join("wip/.gitignore"), "*\n").expect("wip scratch");
    let after = input_hash(std::slice::from_ref(&source)).expect("hash after build");
    assert_eq!(
        before, after,
        "trybuild's wip scratch must not change the inputs"
    );
}

#[test]
fn compiling_lines_are_counted_from_stderr() {
    let stderr = b"   Compiling foo v0.1.0\nwarning: unused\n    Finished `dev` profile\n   \
                   Compiling bar v0.2.0\n";
    assert_eq!(count_compiling(stderr), 2);
}
