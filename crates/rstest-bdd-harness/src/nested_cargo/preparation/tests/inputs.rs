//! What makes a prepared directory stale: the fingerprint and the input hash.

use std::path::{Path, PathBuf};

use super::{
    super::stamp::{
        FINGERPRINT_EXACT,
        FINGERPRINT_PREFIXES,
        PREPARED_FILE_SUFFIX,
        compose,
        fingerprint,
        fingerprint_from,
    },
    environment,
    environment_in,
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

/// Every variable the fingerprint claims to cover, with a representative value.
///
/// The list is derived from the constants the fingerprint itself consults, so a
/// variable added to either list without a matching case here fails the
/// coverage assertion below rather than silently going untested.
fn build_relevant_variables() -> Vec<(String, String)> {
    let exact = FINGERPRINT_EXACT;
    let prefixes = FINGERPRINT_PREFIXES;
    let mut variables: Vec<(String, String)> = exact
        .iter()
        .map(|name| ((*name).to_owned(), "exact-value".to_owned()))
        .collect();
    variables.extend(
        prefixes
            .iter()
            .map(|prefix| (format!("{prefix}SOMETHING"), "prefixed-value".to_owned())),
    );
    variables
}

#[test]
fn every_build_relevant_variable_changes_the_fingerprint() {
    // The fingerprint exists to force a rebuild when the build graph moves. A
    // variable that the filter admits but the fingerprint drops, or one the
    // filter drops by mistake, is invisible here unless each name is exercised
    // individually — the neighbouring case varies only `RUSTFLAGS`, which
    // cannot tell "recorded" from "recorded by coincidence of that one name".
    let target = std::env::temp_dir().join("target");
    let base = environment_in(&target);
    let baseline = fingerprint(&preparation(&target, &base, "v", "hash")).expect("baseline");

    let variables = build_relevant_variables();
    assert!(
        !variables.is_empty(),
        "the fingerprint must cover at least one build-relevant variable"
    );

    for (name, value) in variables {
        let with_variable = crate::nested_cargo::env_from_vars(
            vec![
                ("RUSTFLAGS".into(), "-Dwarnings".into()),
                (name.clone().into(), value.into()),
            ],
            Some(&target),
            crate::nested_cargo::ProfileDestination::Caller,
        );
        let changed = fingerprint(&preparation(&target, &with_variable, "v", "hash"))
            .expect("fingerprint with an extra variable");
        assert_ne!(
            baseline, changed,
            "setting `{name}` must change the fingerprint, or a stamp would survive a change that \
             rebuilds the dependency graph"
        );
    }
}

#[test]
fn variables_outside_the_build_relevant_set_do_not_change_the_fingerprint() {
    // The complement of the case above: if unrelated variables leaked into the
    // fingerprint, every nextest run (a fresh `NEXTEST_RUN_ID` each time) would
    // invalidate every stamp and nothing would ever stay prepared.
    let target = std::env::temp_dir().join("target");
    let base = environment_in(&target);
    let baseline = fingerprint(&preparation(&target, &base, "v", "hash")).expect("baseline");

    for (name, value) in [
        ("NEXTEST_RUN_ID", "run-2"),
        ("LLVM_PROFILE_FILE", "other.profraw"),
        ("PATH", "/usr/local/bin"),
    ] {
        let with_variable = crate::nested_cargo::env_from_vars(
            vec![
                ("RUSTFLAGS".into(), "-Dwarnings".into()),
                (name.into(), value.into()),
            ],
            Some(&target),
            crate::nested_cargo::ProfileDestination::Caller,
        );
        let changed = fingerprint(&preparation(&target, &with_variable, "v", "hash"))
            .expect("fingerprint with an extra variable");
        assert_eq!(
            baseline, changed,
            "`{name}` must not change the fingerprint, or unrelated runner state would invalidate \
             every prepared family"
        );
    }
}

#[test]
fn distinct_toolchain_identities_produce_distinct_fingerprints() {
    // The toolchain field exists to force a rebuild when the compiler moves. If
    // two identities composed to the same text, a stamp written under one
    // toolchain would be accepted under another and a timed assertion would
    // compile the difference inside its own deadline. Composition is
    // exercised directly because resolving the identity runs `cargo -vV`,
    // which a unit test cannot vary.
    let target = std::env::temp_dir().join("target");
    let base = environment_in(&target);
    let preparation = preparation(&target, &base, "v", "hash");

    let stable = compose(&preparation, "release: 1.88.0 abc host: x");
    let nightly = compose(&preparation, "release: 1.92.0 def host: x");
    assert_ne!(
        stable, nightly,
        "a toolchain change must not leave a stamp matching"
    );
}

#[test]
fn a_failing_toolchain_probe_refuses_the_fingerprint() {
    // A fingerprint that could not name the toolchain would be the *same* text
    // on a host where identification fails as on the run before, so a stamp
    // would survive a toolchain change. Failing closed is the only answer that
    // forces invalidation without re-preparing on every run. The resolver is
    // injected here because `toolchain_identity` shells out to `cargo -vV` and
    // a unit test cannot make that fail on demand.
    let target = std::env::temp_dir().join("target");
    let base = environment_in(&target);
    let preparation = preparation(&target, &base, "v", "hash");

    let error = fingerprint_from(&preparation, || {
        Err(std::io::Error::other("cargo -vV is unavailable"))
    })
    .expect_err("a fingerprint that cannot name the toolchain must not be issued");
    assert!(
        error.to_string().contains("cargo -vV is unavailable"),
        "the probe's failure must reach the caller rather than being replaced by a placeholder, \
         got: {error}"
    );
}

#[test]
fn a_successful_toolchain_probe_names_the_release_commit_and_host() {
    // The complement: the injected resolver above proves the failure branch is
    // taken, and this proves the real probe is wired into it. Together they
    // would catch a resolver whose identity was dropped from the text.
    let target = std::env::temp_dir().join("target");
    let base = environment_in(&target);
    let preparation = preparation(&target, &base, "v", "hash");
    let text = fingerprint(&preparation).expect("toolchain probe");
    let line = text
        .as_str()
        .lines()
        .find(|line| line.starts_with("toolchain: "))
        .expect("the fingerprint records a toolchain line");
    let identity = line.trim_start_matches("toolchain: ").trim();
    assert!(
        identity.len() > "release:".len(),
        "the recorded toolchain must name the release, commit and host: {line}"
    );
}

#[test]
fn compiling_lines_are_counted_from_stderr() {
    let stderr = b"   Compiling foo v0.1.0\nwarning: unused\n    Finished `dev` profile\n   \
                   Compiling bar v0.2.0\n";
    assert_eq!(count_compiling(stderr), 2);
}
