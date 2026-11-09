//! What the toolchain probe keeps, and how it fails.

use super::identity_from;

/// A `cargo -vV` invocation's exit outcome and standard output.
///
/// Passed as a flag rather than a `process::ExitStatus` because a synthetic
/// non-zero status cannot be constructed portably.
const PROBE_OK: bool = true;
const PROBE_FAILED: bool = false;

#[test]
fn the_toolchain_probe_keeps_only_release_commit_and_host() {
    // The identity decides whether a stamp survives a compiler change, so the
    // three fields must be kept and everything else dropped: a `cargo -vV`
    // line that varies per invocation would otherwise invalidate every stamp.
    let identity = identity_from(
        PROBE_OK,
        "cargo 1.88.0 (1234abcd 2025-06-01)\nrelease: 1.88.0\ncommit-hash: 1234abcd\nhost: \
         x86_64-unknown-linux-gnu\ncargo-features: some-unstable-flag\n",
    )
    .expect("a complete probe identifies the toolchain");
    assert_eq!(
        identity, "release: 1.88.0 commit-hash: 1234abcd host: x86_64-unknown-linux-gnu",
        "the identity must name exactly the release, commit and host"
    );
    assert!(
        !identity.contains("cargo-features"),
        "an unstable feature list must not reach the fingerprint: {identity}"
    );
}

#[test]
fn a_toolchain_probe_that_exits_non_zero_is_an_error() {
    // A fingerprint that could not name the toolchain would be the *same* text
    // on a host where identification fails as on the run before, so a stamp
    // would survive a toolchain change. Failing closed is the only answer that
    // forces invalidation without re-preparing on every run.
    let error = identity_from(PROBE_FAILED, "release: 1.88.0\n")
        .expect_err("a failed probe must not yield an identity");
    assert!(
        error.to_string().contains("cargo -vV"),
        "the failure must name the probe, got: {error}"
    );
}

#[test]
fn a_toolchain_probe_without_the_three_fields_is_an_error() {
    // The complement of the exit-status case: a command that succeeds but
    // prints nothing usable must not be read as an identity either, or two
    // different toolchains could compose the same empty text.
    let error = identity_from(PROBE_OK, "cargo 1.88.0\nsomething else\n")
        .expect_err("an unusable probe must not yield an identity");
    assert!(
        error.to_string().contains("release, commit-hash or host"),
        "the failure must name the missing fields, got: {error}"
    );
}
