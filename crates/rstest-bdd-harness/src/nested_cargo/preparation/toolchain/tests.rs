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

use std::{
    io,
    path::{Path, PathBuf},
    time::Duration,
};

use super::{PROBE_TIMEOUT, compose_identity, identity_from, probe_bound, resolve_compiler_path};
use crate::nested_cargo::Deadline;

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

/// A nested build's working directory for the resolution cases.
///
/// Absolute, so the expected results are the same on every host: a relative
/// fixture would have the expectations reading the process's own directory.
fn nested_directory() -> PathBuf { Path::new("/srv/fixture").to_path_buf() }

#[test]
fn a_relative_compiler_override_resolves_beneath_the_nested_directory() {
    // Cargo resolves a relative `RUSTC` against the working directory of the
    // command that reads it, and the nested build runs in the fixture, not in
    // the test process. Probing `./tools/rustc` from the test process would
    // identify a different compiler when both paths exist, and fail outright
    // when the compiler exists only beneath the fixture — either way refusing
    // the timed assertion or admitting it against the wrong compiler.
    let resolved = resolve_compiler_path("./tools/rustc".as_ref(), &nested_directory());
    assert_eq!(
        resolved,
        nested_directory().join("tools").join("rustc"),
        "a relative override must be read from the directory the nested build runs in"
    );
}

#[test]
fn an_absolute_compiler_override_is_used_unchanged() {
    // An absolute `RUSTC` already names one place for both the probe and the
    // build, so joining it to the nested directory would be wrong rather than
    // redundant: a joined absolute path is the absolute path, but a joining
    // implementation that treated it as relative would silently move the
    // probe to `<fixture>/srv/toolchain/rustc`.
    let absolute = Path::new("/srv/toolchain/rustc");
    assert_eq!(
        resolve_compiler_path(absolute.as_os_str(), &nested_directory()),
        absolute,
        "an absolute override must not be re-rooted"
    );
}

#[test]
fn a_bare_compiler_name_stays_a_name_for_path_lookup() {
    // Cargo also searches `PATH` for a `RUSTC` value that names no location.
    // Turning `rustc` into `<fixture>/rustc` would make the probe look for a
    // file that need not exist while a perfectly good compiler sits on `PATH`.
    let resolved = resolve_compiler_path("rustc".as_ref(), &nested_directory());
    assert_eq!(
        resolved,
        PathBuf::from("rustc"),
        "a bare name must stay a name for the probe to search `PATH`"
    );
}

#[test]
fn a_dot_prefixed_compiler_override_is_resolved_rather_than_taken_as_a_name() {
    // The distinction is a path separator, not the leading dot: `.cargo/rustc`
    // names a location and must be joined, while `rustc` does not. A rule that
    // keyed on the dot would get both of these wrong the moment a fixture put
    // its toolchain beside its manifest.
    let resolved = resolve_compiler_path(".cargo/rustc".as_ref(), &nested_directory());
    assert_eq!(
        resolved,
        nested_directory().join(".cargo").join("rustc"),
        "a value naming a location must be resolved, dot or not"
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

#[test]
fn a_probe_without_a_deadline_gets_its_own_limit() {
    // The ordinary family: nothing else runs in the gate's budget, so the
    // per-probe limit is the whole answer. A path that consulted a deadline it
    // was not given, or that reported zero, would refuse every probe on a
    // family with no period to overrun.
    let bound = probe_bound(None, CARGO).expect("no deadline cannot refuse");
    assert_eq!(
        bound, PROBE_TIMEOUT,
        "a family without a shared budget keeps the per-probe limit"
    );
}

#[test]
fn a_probe_under_a_live_deadline_gets_the_smaller_of_the_two() {
    // The slow-success remedy: the budget's remainder is folded in beside the
    // per-probe limit. With an hour left and a one-minute limit the limit wins,
    // which is what makes a sequence of healthy probes no slower than before.
    let deadline = Deadline::starting_now(Duration::from_secs(3600));
    let bound = probe_bound(Some(deadline), COMPILER).expect("an hour is live");
    assert_eq!(
        bound, PROBE_TIMEOUT,
        "a per-probe limit below the remainder must be taken as-is"
    );
    // And with less than a minute left the remainder wins, so two probes
    // cannot each take a fresh minute out of a budget that has almost none.
    let tight = Deadline::starting_now(Duration::from_secs(5));
    let bound = probe_bound(Some(tight), COMPILER).expect("five seconds is live");
    assert!(
        bound <= Duration::from_secs(5),
        "a probe may not be given more than the budget has left: {bound:?}"
    );
}

#[test]
fn a_probe_under_a_spent_deadline_is_refused_by_name() {
    // The case the deadline was introduced for. A query that succeeded just
    // inside its own limit used to hand the probes a budget already spent, and
    // the runner killed the test before either reported. The refusal has to
    // name the probe so a reader knows which command never started.
    let error = probe_bound(Some(Deadline::starting_now(Duration::ZERO)), COMPILER)
        .expect_err("a spent budget must refuse the probe");
    assert_eq!(
        error.kind(),
        io::ErrorKind::TimedOut,
        "a refusal is a timeout, not a spawn failure: {error}"
    );
    let text = error.to_string();
    assert!(
        text.contains("`rustc -vV`"),
        "the refusal must name the probe that never started: {text}"
    );
}
