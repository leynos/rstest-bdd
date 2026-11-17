//! What each toolchain probe keeps, and how it fails.
//!
//! Exercises [`identity_from`] and [`compose_identity`] from the parent
//! [`toolchain`](super) module: the pure halves that turn two completed `-vV`
//! invocations into the identity text a fingerprint carries, one for Cargo and
//! one for the compiler it now also probes. The parent's subprocess half —
//! `toolchain_identity`, which spawns both probes — is the boundary and is not
//! exercised here. Keeping these cases on the text alone is what lets them state
//! a synthetic non-zero exit, which no portable `process::ExitStatus` can
//! construct.

use super::{compose_identity, identity_from};

/// A `-vV` invocation's exit outcome and standard output.
///
/// Passed as a flag rather than a `process::ExitStatus` because a synthetic
/// non-zero status cannot be constructed portably.
const PROBE_OK: bool = true;
const PROBE_FAILED: bool = false;

/// The two programs the parent probes, spelled as their failures name them.
const CARGO: &str = "cargo";
const COMPILER: &str = "rustc";

#[test]
fn the_toolchain_probe_keeps_only_release_commit_and_host() {
    // The identity decides whether a stamp survives a compiler change, so the
    // three fields must be kept and everything else dropped: a `-vV` line that
    // varies per invocation would otherwise invalidate every stamp.
    let identity = identity_from(
        CARGO,
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
fn the_compiler_probe_is_identified_by_its_own_program() {
    // `rustc -vV` prints the same three field names as `cargo -vV`, so the same
    // reduction applies; what must differ is which program a failure names. A
    // message that said `cargo` for a compiler failure would send a reader to
    // the wrong binary when the compiler path was the one that was wrong.
    let identity = identity_from(
        COMPILER,
        PROBE_OK,
        "rustc 1.88.0 (1234abcd 2025-06-01)\nrelease: 1.88.0\ncommit-hash: 1234abcd\nhost: \
         x86_64-unknown-linux-gnu\n",
    )
    .expect("a complete compiler probe identifies the compiler");
    assert_eq!(
        identity, "release: 1.88.0 commit-hash: 1234abcd host: x86_64-unknown-linux-gnu",
        "the compiler's identity must be the same three fields"
    );
}

#[test]
fn the_two_probed_identities_stay_distinguishable() {
    // The compiler is probed separately because a compiler can change while
    // Cargo does not. That only forces a rebuild if the two identities stay
    // apart in the joined text: were they concatenated with a space, the pair
    // ("a b", "c") and the pair ("a", "b c") would compose the same line, and a
    // changed compiler could then leave a stamp matching.
    let one = compose_identity("release: 1.88.0 host: x", "release: 1.88.0 host: x");
    let other = compose_identity("release: 1.88.0", "host: x release: 1.88.0 host: x");
    assert_ne!(
        one, other,
        "each identity must occupy its own line, got {one:?} and {other:?}"
    );
    assert_eq!(
        one.lines().count(),
        2,
        "the joined text must be exactly two lines: {one:?}"
    );
    assert!(
        one.starts_with("cargo: ") && one.contains("\nrustc: "),
        "each line must name the program it identifies: {one:?}"
    );
}

#[test]
fn a_toolchain_probe_that_exits_non_zero_is_an_error() {
    // A fingerprint that could not name the toolchain would be the *same* text
    // on a host where identification fails as on the run before, so a stamp
    // would survive a toolchain change. Failing closed is the only answer that
    // forces invalidation without re-preparing on every run.
    let error = identity_from(CARGO, PROBE_FAILED, "release: 1.88.0\n")
        .expect_err("a failed probe must not yield an identity");
    assert!(
        error.to_string().contains("cargo -vV"),
        "the failure must name the probe, got: {error}"
    );
}

#[test]
fn a_failing_compiler_probe_names_the_compiler() {
    // The compiler probe is the one that matters for an in-place compiler
    // change, so its own failure has to be attributable. A message naming Cargo
    // would report the wrong cause for the very case the second probe exists
    // to catch.
    let error = identity_from(COMPILER, PROBE_FAILED, "release: 1.88.0\n")
        .expect_err("a failed compiler probe must not yield an identity");
    assert!(
        error.to_string().contains("rustc -vV"),
        "the compiler's failure must name the compiler, got: {error}"
    );
}

#[test]
fn a_toolchain_probe_without_the_three_fields_is_an_error() {
    // The complement of the exit-status case: a command that succeeds but
    // prints nothing usable must not be read as an identity either, or two
    // different toolchains could compose the same empty text.
    let error = identity_from(CARGO, PROBE_OK, "cargo 1.88.0\nsomething else\n")
        .expect_err("an unusable probe must not yield an identity");
    assert!(
        error.to_string().contains("release, commit-hash or host"),
        "the failure must name the missing fields, got: {error}"
    );
}
