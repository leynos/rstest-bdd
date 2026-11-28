//! The denial witness: a child not told to be offline still cannot fetch.
//!
//! Offline mode makes Cargo refuse to use the network; it does not make the
//! network unreachable. This module removes `CARGO_NET_OFFLINE` from the child
//! it starts, so the child is free to try, and asserts that the attempt fails
//! for want of a network rather than for want of a package.
//!
//! The scratch crate is built through a capability on the temporary directory
//! it owns, with every path relative to it, so a mistake in a path cannot write
//! outside the tree this test created.

use std::{env, process::Command};

use cap_std::ambient_authority;

/// The scratch manifest: a package name that cannot exist in any registry.
///
/// An unregistrable name is what makes the witness uncacheable. A name that
/// merely happened to be absent from this host's cache could be satisfied by a
/// later fetch; this one never can.
const SCRATCH_MANIFEST: &str = concat!(
    "[package]\n",
    "name = \"offline-denial-witness\"\n",
    "version = \"0.0.0\"\n",
    "edition = \"2024\"\n\n",
    "[dependencies]\n",
    "rstest-bdd-offline-witness-does-not-exist = \"0.0.0\"\n",
);

/// Write the scratch crate and return the directory that owns it.
fn write_scratch_crate() -> std::io::Result<tempfile::TempDir> {
    let scratch = tempfile::tempdir()?;
    // The byte-path handle, because `tempfile` yields a `std` path and the
    // scratch tree is built entirely through this capability.
    let dir = cap_std::fs::Dir::open_ambient_dir(scratch.path(), ambient_authority())?;
    dir.create_dir_all("src")?;
    dir.write("Cargo.toml", SCRATCH_MANIFEST)?;
    dir.write("src/lib.rs", "")?;
    Ok(scratch)
}

/// Run `cargo fetch --locked` in *dir* with offline mode removed.
///
/// The child is bounded by the shared nested-Cargo runner: a fetch that waits
/// on the package-cache lock would otherwise hang the test with no report,
/// which is exactly the failure mode the bound exists to turn into a named
/// error.
fn fetch_without_offline_mode(dir: &std::path::Path) -> std::io::Result<std::process::Output> {
    let mut cmd = Command::new(env!("CARGO"));
    cmd.current_dir(dir)
        .env_remove("CARGO_NET_OFFLINE")
        .args(["fetch", "--locked"]);
    rstest_bdd_harness::nested_cargo::run_bounded(&mut cmd)
}

/// Whether this run is the one that executes inside the wrapper.
///
/// Only the `offline-proof` profile runs the network namespace. Outside it the
/// child would really reach the network, find the package missing, and report
/// the *other* message, so the test would accuse the code of a defect that is
/// the lane's own. Gating on the variable rather than on the platform keeps the
/// cost where the proof is.
fn is_proof_run() -> bool { env::var("NEXTEST_PROFILE").as_deref() == Ok("offline-proof") }

/// A child that is free to use the network, and has none, must fail on that.
///
/// The two messages are distinguishable and the test asserts both halves:
/// `unable to update registry` is what a denied child reports, and
/// `no matching package` is what a child that reached the index reports. The
/// second assertion is the one that makes the first meaningful: a child that
/// never tried could not print either.
#[test]
fn a_denied_child_cannot_reach_the_registry() {
    if !is_proof_run() {
        return;
    }

    let scratch = write_scratch_crate().expect("scratch crate");
    let output = fetch_without_offline_mode(scratch.path()).expect("spawn cargo fetch");
    let combined = format!(
        "{}\n{}",
        String::from_utf8_lossy(&output.stdout),
        String::from_utf8_lossy(&output.stderr),
    );

    assert!(
        !output.status.success(),
        "the denied child must fail rather than resolve the package; it printed: {combined}"
    );
    assert!(
        combined.contains("unable to update registry"),
        "a child with no network must fail to update the registry; it printed: {combined}"
    );
    assert!(
        !combined.contains("no matching package"),
        concat!(
            "`no matching package` means the child reached the index, so the namespace denied ",
            "nothing; it printed: {}",
        ),
        combined
    );
}
