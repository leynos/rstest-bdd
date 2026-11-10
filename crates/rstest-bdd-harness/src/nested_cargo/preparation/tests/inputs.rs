//! What makes a prepared directory stale: the fingerprint and the input hash.
//!
//! The input hash sees what a fixture's own source tree contributes. Cargo's
//! configuration is a separate class of input with its own discovery rules and
//! its own failure modes, so it lives in `cargo_config`.

use std::path::Path;

use super::{
    super::stamp::{
        FINGERPRINT_EXACT,
        PREPARED_FILE_SUFFIX,
        compose,
        escape,
        fingerprint_with,
        is_fingerprint_variable,
    },
    environment,
    environment_in,
    fixture_package,
    preparation,
};
use crate::nested_cargo::{count_compiling, input_hash};

/// A fixed toolchain identity for cases that are not about the toolchain.
///
/// These tests ask whether an environment variable changes the fingerprint, so
/// the identity has to be held constant or a change in the real host toolchain
/// would be indistinguishable from the variable's effect. Resolving the real
/// identity would also spawn `cargo -vV` in every case.
///
/// The `io::Result` is never `Err`; it is the shape the gate's probe takes, so
/// that a probe which can fail — the production one does — shares the same
/// parameter.
#[expect(
    clippy::unnecessary_wraps,
    reason = "the gate's toolchain probe is fallible by contract"
)]
fn recorded_toolchain() -> std::io::Result<String> {
    Ok(String::from("release: 1.88.0 commit-hash: a host: x"))
}

#[test]
fn fingerprint_excludes_runner_and_coverage_variables() {
    let environment = environment();
    let text = fingerprint_with(
        &preparation(
            Path::new("/tmp/target"),
            &environment,
            "all-features",
            "abc",
        ),
        recorded_toolchain,
    )
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
    // Every recorded value is escaped onto one line, and a Windows path is
    // full of backslashes, so the expectation has to be escaped the same way.
    // Comparing against the raw path asserts that escaping does not happen.
    assert!(
        text.contains(&format!("CARGO_TARGET_DIR={}", escape(&target))),
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
    let one = fingerprint_with(
        &preparation(
            Path::new("/tmp/target"),
            &environment,
            "all-features",
            "abc",
        ),
        recorded_toolchain,
    )
    .expect("fingerprint");
    let other = fingerprint_with(
        &preparation(
            Path::new("/tmp/target"),
            &environment,
            "no-default-features",
            "abc",
        ),
        recorded_toolchain,
    )
    .expect("fingerprint");
    let third = fingerprint_with(
        &preparation(
            Path::new("/tmp/target"),
            &environment,
            "all-features",
            "def",
        ),
        recorded_toolchain,
    )
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

/// Every variable class the fingerprint must cover, with a representative value.
///
/// Written out independently of [`FINGERPRINT_EXACT`] and
/// [`FINGERPRINT_PREFIXES`] rather than derived from them. A derived list
/// cannot detect a removal: deleting `RUSTUP_TOOLCHAIN` from the production
/// constant would also delete its case, so the coverage test would keep passing
/// over a fingerprint that had stopped covering the toolchain selector. The
/// explicit list makes the constant's contents a claim the test checks.
fn build_relevant_variables() -> Vec<(String, String)> {
    [
        // Value that differs from what `environment_in` already sets. Deciding
        // a variable is covered means changing its *value*: re-setting the
        // baseline value would compare equal and pass for the wrong reason.
        ("RUSTFLAGS", "-Dwarnings -Cdebuginfo=1"),
        ("CARGO_ENCODED_RUSTFLAGS", "-Dwarnings"),
        ("CARGO_INCREMENTAL", "0"),
        ("RUSTDOCFLAGS", "-Dwarnings"),
        ("RUSTUP_TOOLCHAIN", "nightly"),
        ("RUSTC_WRAPPER", "sccache"),
        ("RUSTDOC_WRAPPER", "wrapper"),
        ("CARGO_BUILD_TARGET_DIR", "/tmp/other-target"),
        ("CARGO_PROFILE_DEV_OPT_LEVEL", "2"),
        ("CARGO_TARGET_X86_64_UNKNOWN_LINUX_GNU_LINKER", "clang"),
    ]
    .into_iter()
    .map(|(name, value)| (name.to_owned(), value.to_owned()))
    .collect()
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
    let baseline = fingerprint_with(
        &preparation(&target, &base, "v", "hash"),
        recorded_toolchain,
    )
    .expect("baseline");

    let variables = build_relevant_variables();
    // The explicit list above and the production constants are two independent
    // statements of the same policy, so each must imply the other. A name in
    // the list but not in the constants is a stale expectation; a constant with
    // no case is an untested claim.
    for name in &FINGERPRINT_EXACT {
        assert!(
            variables.iter().any(|(candidate, _)| candidate == name),
            "`{name}` is fingerprint-relevant but has no case in this test"
        );
    }
    for (name, _) in &variables {
        assert!(
            is_fingerprint_variable(name),
            "`{name}` is asserted relevant here but the fingerprint does not admit it"
        );
    }

    for (name, value) in variables {
        let with_variable = crate::nested_cargo::env_from_vars(
            vec![
                ("RUSTFLAGS".into(), "-Dwarnings".into()),
                (name.clone().into(), value.into()),
            ],
            Some(&target),
            crate::nested_cargo::ProfileDestination::Caller,
        );
        let changed = fingerprint_with(
            &preparation(&target, &with_variable, "v", "hash"),
            recorded_toolchain,
        )
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
    let baseline = fingerprint_with(
        &preparation(&target, &base, "v", "hash"),
        recorded_toolchain,
    )
    .expect("baseline");

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
        let changed = fingerprint_with(
            &preparation(&target, &with_variable, "v", "hash"),
            recorded_toolchain,
        )
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
fn a_value_containing_a_line_break_cannot_forge_a_field() {
    // Every field is one line and the fingerprint is compared as text, so a
    // value carrying a line break could otherwise write what looks like the
    // next field. Two genuinely different builds would then share a stamp,
    // and the invalidation the fingerprint exists to force would not happen.
    let target = std::env::temp_dir().join("target");
    let environment = environment_in(&target);
    // These two descriptions are genuinely different — one has an empty input
    // hash, the other an empty variant — and without escaping they compose the
    // same three trailing lines: `variant: v`, `inputs: h` and `inputs: `,
    // because the first moves its tail into the variant and the second moves
    // its tail into the inputs. A stamp written for one would then match the
    // other, so a changed fixture would keep its prepared artefacts.
    let tail_in_variant = preparation(&target, &environment, "v\ninputs: h", "");
    let tail_in_inputs = preparation(&target, &environment, "v", "h\ninputs: ");

    let one = compose(&tail_in_variant, "release: 1.88.0 host: x");
    let other = compose(&tail_in_inputs, "release: 1.88.0 host: x");
    assert_ne!(
        one,
        other,
        "a newline in a value must not be able to forge the following field:\n{}",
        one.as_str()
    );
    for text in [one.as_str(), other.as_str()] {
        assert_eq!(
            text.lines().count(),
            environment
                .iter()
                .filter(|(key, _)| is_fingerprint_variable(&key.to_string_lossy()))
                .count()
                + 4,
            "each field must occupy exactly one line, got:\n{text}"
        );
    }
}

#[test]
fn the_recorded_toolchain_reaches_the_fingerprint() {
    // The probe and the composition are tested separately above; this ties
    // them together. If `fingerprint` stopped forwarding the resolved identity,
    // the two branches would still pass while every stamp became
    // toolchain-blind.
    let target = std::env::temp_dir().join("target");
    let base = environment_in(&target);
    let prepared = preparation(&target, &base, "v", "hash");

    let first = compose(&prepared, "release: 1.88.0 commit-hash: a host: x");
    let second = compose(&prepared, "release: 1.92.0 commit-hash: b host: x");
    assert_ne!(first, second);
    assert!(
        first
            .as_str()
            .contains("release: 1.88.0 commit-hash: a host: x")
    );
}

#[test]
fn compiling_lines_are_counted_from_stderr() {
    let stderr = b"   Compiling foo v0.1.0\nwarning: unused\n    Finished `dev` profile\n   \
                   Compiling bar v0.2.0\n";
    assert_eq!(count_compiling(stderr), 2);
}
