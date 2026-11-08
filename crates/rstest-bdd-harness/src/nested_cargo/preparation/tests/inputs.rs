//! What makes a prepared directory stale: the fingerprint and the input hash.

use std::path::{Path, PathBuf};

use super::{
    super::stamp::{PREPARED_FILE_SUFFIX, fingerprint},
    environment,
    fixture_package,
    preparation,
};
use crate::nested_cargo::{cargo_config_inputs, count_compiling, input_hash};

#[test]
fn fingerprint_excludes_runner_and_coverage_variables() {
    let environment = environment();
    let text = fingerprint(&preparation(
        Path::new("/tmp/target"),
        &environment,
        "all-features",
        "abc",
    ))
    .expect("the fingerprint must name the toolchain");
    let text = text.as_str();
    // The fingerprint records whatever the child receives, and the environment
    // resolves `CARGO_TARGET_DIR` once before that. Asserting on the resolved
    // value rather than on the literal above keeps the check honest on every
    // platform: on Windows `/tmp/target` has no drive letter, so it is a
    // relative path and resolving it is exactly the behaviour under test.
    let target = environment
        .get("CARGO_TARGET_DIR")
        .expect("the filtered environment records a target directory")
        .to_string_lossy()
        .into_owned();
    assert!(
        Path::new(&target).is_absolute(),
        "the child must receive an absolute target directory, got: {target}"
    );
    assert!(text.contains("RUSTFLAGS=-Dwarnings"));
    assert!(
        text.contains(&format!("CARGO_TARGET_DIR={target}")),
        "the fingerprint must record the resolved target directory, got:\n{text}"
    );
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
    ))
    .expect("fingerprint");
    let other = fingerprint(&preparation(
        Path::new("/tmp/target"),
        &environment,
        "no-default-features",
        "abc",
    ))
    .expect("fingerprint");
    let third = fingerprint(&preparation(
        Path::new("/tmp/target"),
        &environment,
        "all-features",
        "def",
    ))
    .expect("fingerprint");
    assert_ne!(one, other);
    assert_ne!(one, third);
}

#[test]
fn input_hash_ignores_stamps_and_build_output() {
    let (_directory, source) = fixture_package();
    std::fs::create_dir_all(source.join("features")).expect("features dir");
    std::fs::write(source.join("features/a.feature"), "Feature: a\n").expect("feature");
    let before = input_hash(std::slice::from_ref(&source)).expect("initial hash");

    std::fs::create_dir_all(source.join("target/debug")).expect("target dir");
    std::fs::write(source.join("target/debug/artifact"), "binary").expect("artefact");
    std::fs::write(
        source.join(format!(".unit{PREPARED_FILE_SUFFIX}")),
        "different fingerprint",
    )
    .expect("stamp");
    let after = input_hash(std::slice::from_ref(&source)).expect("hash after build");
    assert_eq!(before, after, "build output must not change the inputs");

    std::fs::write(source.join("features/a.feature"), "Feature: changed\n").expect("edit");
    let changed = input_hash(&[source]).expect("hash after edit");
    assert_ne!(before, changed, "fixture edits must change the inputs");
}

#[test]
fn input_hash_ignores_trybuild_wip_scratch() {
    // `trybuild` writes `wip/` beside the crate it tests, not inside the
    // target directory, and it creates that directory during the very build a
    // preparation stamps. Hashing it would invalidate every stamp the moment
    // it was written, because the hash is taken before that build runs.
    let (_directory, source) = fixture_package();
    let before = input_hash(std::slice::from_ref(&source)).expect("initial hash");

    std::fs::create_dir_all(source.join("wip")).expect("wip dir");
    std::fs::write(source.join("wip/.gitignore"), "*\n").expect("wip scratch");
    let after = input_hash(std::slice::from_ref(&source)).expect("hash after build");
    assert_eq!(
        before, after,
        "trybuild's wip scratch must not change the inputs"
    );
}

#[test]
fn cargo_configuration_is_an_input() {
    // Cargo discovers configuration by walking up from a nested build's working
    // directory, so the workspace `.cargo/config.toml` applies to every fixture
    // build here. Changing its profile options or linker flags invalidates
    // dependency artefacts without changing a fixture source, so a stamp that
    // ignored it would let a timed assertion pay for the rebuild.
    let workspace = tempfile::tempdir().expect("workspace dir");
    std::fs::create_dir(workspace.path().join(".cargo")).expect("cargo dir");
    let config = workspace.path().join(".cargo/config.toml");
    std::fs::write(&config, "[profile.dev]\nincremental = false\n").expect("config");

    assert_eq!(
        cargo_config_inputs(workspace.path()).expect("config inputs"),
        vec![config.clone()]
    );

    let before = input_hash(&cargo_config_inputs(workspace.path()).expect("config inputs"))
        .expect("hash before");
    std::fs::write(&config, "[profile.dev]\nincremental = true\n").expect("config edit");
    let after = input_hash(&cargo_config_inputs(workspace.path()).expect("config inputs"))
        .expect("hash after");
    assert_ne!(
        before, after,
        "a configuration-only change must invalidate the prepared artefacts"
    );
}

#[test]
fn missing_cargo_configuration_is_not_an_input() {
    // A checkout may commit neither accepted file name; naming a missing root
    // would make every preparation fail on a read error instead of treating the
    // configuration as absent.
    let workspace = tempfile::tempdir().expect("workspace dir");
    assert_eq!(
        cargo_config_inputs(workspace.path()).expect("config inputs"),
        [] as [PathBuf; 0]
    );
}

#[test]
#[cfg(unix)]
fn an_unreadable_cargo_configuration_is_an_error() {
    // A configuration path that exists but cannot be inspected must not be
    // reported as absent. Reporting it absent would drop the configuration
    // from the fingerprint, so a stamp would survive a change the build
    // notices — exactly the invalidation this input exists to force.
    //
    // A symlink loop reaches that state without any permission setup, which
    // would not hold when the suite runs as root. It is a Unix-only fixture
    // because creating a symlink on Windows needs a privilege the test runner
    // does not have.
    let workspace = tempfile::tempdir().expect("workspace dir");
    std::fs::create_dir(workspace.path().join(".cargo")).expect("cargo dir");
    let looped = workspace.path().join(".cargo/config.toml");
    std::os::unix::fs::symlink(&looped, &looped).expect("symlink loop");

    let error = cargo_config_inputs(workspace.path())
        .expect_err("an uninspectable configuration must not pass as absent");
    assert_ne!(
        error.kind(),
        std::io::ErrorKind::NotFound,
        "a loop is not an absence: {error}"
    );
}

#[test]
fn compiling_lines_are_counted_from_stderr() {
    let stderr = b"   Compiling foo v0.1.0\nwarning: unused\n    Finished `dev` profile\n   \
                   Compiling bar v0.2.0\n";
    assert_eq!(count_compiling(stderr), 2);
}
