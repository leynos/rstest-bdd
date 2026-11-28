//! Contract that a Cargo test process starts with offline mode already set.
//!
//! `EP-M2` makes every spawn this repository owns say "offline" itself. That
//! leaves the spawns it does not own: a third-party crate's nested `cargo`, a
//! build script, a helper that shells out without being told. The only place
//! that can cover all of them at once is Cargo's own environment, because Cargo
//! applies `.cargo/config.toml`'s `[env]` table to every process it starts —
//! rustc, build scripts, and the test binaries themselves.
//!
//! This test reads the process it is running in. It therefore fails whenever
//! the configuration entry is missing, whatever else the tree does, and it
//! names the file to edit rather than the symptom.
//!
//! It also pins the shape of the entry: `force = true` is deliberately not set,
//! because a developer who needs to fetch a new dependency must be able to run
//! one command with `CARGO_NET_OFFLINE=false` in front of it. Forcing the value
//! would remove that escape without adding any safety the default does not
//! already give.

use std::env;

use camino::Utf8PathBuf;

/// The repository root, from this crate's manifest directory.
fn repository_root() -> Utf8PathBuf {
    let manifest_dir = Utf8PathBuf::from(env!("CARGO_MANIFEST_DIR"));
    let Some(root) = manifest_dir.parent().and_then(camino::Utf8Path::parent) else {
        panic!("the harness crate lives two levels below the repository root");
    };
    root.to_path_buf()
}

#[test]
fn the_test_process_starts_offline() {
    assert_eq!(
        env::var("CARGO_NET_OFFLINE").as_deref(),
        Ok("true"),
        "every test process must start offline so a spawn this repository does \
         not construct cannot reach the registry; set `CARGO_NET_OFFLINE = \
         \"true\"` under `[env]` in .cargo/config.toml"
    );
}

#[test]
fn the_offline_default_is_not_forced() {
    let config = repository_root().join(".cargo/config.toml");
    let contents = std::fs::read_to_string(config.as_std_path())
        .unwrap_or_else(|err| panic!("cannot read {config}: {err}"));
    let parsed: toml::Value =
        toml::from_str(&contents).unwrap_or_else(|err| panic!("{config} is not valid TOML: {err}"));

    let entry = parsed
        .get("env")
        .and_then(|env| env.get("CARGO_NET_OFFLINE"))
        .unwrap_or_else(|| {
            panic!(
                "{config} must declare `CARGO_NET_OFFLINE` under `[env]`, or a \
                 test spawn this repository does not construct reaches the \
                 network"
            )
        });

    // Cargo accepts either the bare string form or a table, so both are read.
    // The bare form is what the file uses; the table is accepted because
    // someone may later add `relative` or a comment beside the value.
    let (value, force) = match entry {
        toml::Value::String(value) => (Some(value.as_str()), None),
        toml::Value::Table(table) => (
            table.get("value").and_then(toml::Value::as_str),
            table.get("force").and_then(toml::Value::as_bool),
        ),
        other => panic!("`CARGO_NET_OFFLINE` must be a string or a table, got {other:?}"),
    };

    assert_eq!(
        value,
        Some("true"),
        "the offline default must be the string \"true\""
    );
    assert_ne!(
        force,
        Some(true),
        "the default must stay overridable, so a developer can fetch a new \
         dependency with `CARGO_NET_OFFLINE=false <command>`"
    );
}

/// The denial proof: a child that is *not* told to be offline still cannot fetch.
///
/// Offline mode makes Cargo refuse to use the network; it does not make the
/// network unreachable. This test removes `CARGO_NET_OFFLINE` from the child it
/// starts, so the child is free to try, and asserts that the attempt fails for
/// want of a network rather than for want of a package. The scratch manifest
/// names a dependency that cannot exist in any registry, so the child cannot
/// answer from a warm cache: it has to ask the index, and a denied namespace is
/// what stops it.
///
/// Only the `offline-proof` profile runs the wrapper, so the ordinary suite
/// skips this test: outside that profile the child would really reach the
/// network, find the package missing, and report the *other* message. Gating on
/// the variable rather than on the platform keeps the cost where the proof is.
///
/// The two messages are distinguishable and the test asserts both halves:
/// `unable to update registry` is what a denied child reports, and
/// `no matching package` is what a child that reached the index reports.
#[test]
#[cfg(target_os = "linux")]
fn a_denied_child_cannot_reach_the_registry() {
    use std::{fs, process::Command};

    if env::var("NEXTEST_PROFILE").as_deref() != Ok("offline-proof") {
        return;
    }

    let scratch = tempfile::tempdir().expect("scratch directory");
    let src = scratch.path().join("src");
    fs::create_dir_all(&src).expect("scratch source directory");
    fs::write(
        scratch.path().join("Cargo.toml"),
        concat!(
            "[package]\n",
            "name = \"offline-denial-witness\"\n",
            "version = \"0.0.0\"\n",
            "edition = \"2024\"\n\n",
            "[dependencies]\n",
            "rstest-bdd-offline-witness-does-not-exist = \"0.0.0\"\n",
        ),
    )
    .expect("scratch manifest");
    fs::write(src.join("lib.rs"), "").expect("scratch source");

    let output = Command::new(env!("CARGO"))
        .current_dir(scratch.path())
        .env_remove("CARGO_NET_OFFLINE")
        .args(["fetch", "--locked"])
        .output()
        .expect("spawn cargo fetch");
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
        "`no matching package` means the child reached the index, so the \
         namespace denied nothing; it printed: {combined}"
    );
}
