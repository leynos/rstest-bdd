//! The CLI must fail closed on a dependency it cannot resolve offline.
//!
//! `cli_child_environment` injects `CARGO_NET_OFFLINE`, and the environment
//! map is an internal representation: a test that only reads the map would
//! still pass if the CLI dropped the variable before spawning its nested
//! `cargo`. This module observes the effect instead, through a manifest whose
//! dependency name cannot exist in any registry, so the nested `cargo
//! metadata` has to reach the index however warm the host's cache is.
//!
//! The test lives here rather than in `cli.rs` to keep that file within the
//! repository's per-file line budget; `cli.rs` attaches it with `#[path]` so
//! Cargo does not also compile it as a second test target.

use std::{fs, path::Path};

use eyre::{Context, Result};
use rstest_bdd_harness::nested_cargo;
use serial_test::serial;

use super::{cli_child_environment, locate_or_build_cargo_bdd_command};

/// Write a scratch crate whose only dependency cannot exist.
///
/// The name is deliberately unregistrable, so this manifest can never resolve
/// from a warm cache: it makes Cargo ask the registry however much of the graph
/// a host has already built, which is what gives the test below something to
/// observe on any machine.
fn write_unresolvable_manifest(dir: &Path) -> Result<()> {
    fs::create_dir_all(dir.join("src")).wrap_err("scratch crate source directory")?;
    fs::write(
        dir.join("Cargo.toml"),
        concat!(
            "[package]\n",
            "name = \"cargo-bdd-offline-witness\"\n",
            "version = \"0.0.0\"\n",
            "edition = \"2024\"\n\n",
            "[dependencies]\n",
            "rstest-bdd-offline-witness-does-not-exist = \"0.0.0\"\n",
        ),
    )
    .wrap_err("scratch crate manifest")?;
    fs::write(dir.join("src/lib.rs"), "").wrap_err("scratch crate source")?;
    Ok(())
}

/// The CLI must fail closed rather than resolve a package over the network.
///
/// `steps` builds the manifest's test binaries, so Cargo has to resolve the
/// dependency graph first. With offline mode in the child environment that
/// mutation is refused, and Cargo names the refusal: the `offline mode` note
/// appears only when resolution ran offline. Removing the injected variable
/// makes this test fail — the witness is uncacheable, so nothing else can
/// mask it.
#[test]
#[serial]
fn cargo_bdd_fails_closed_without_cached_packages() -> Result<()> {
    let scratch = tempfile::tempdir().wrap_err("scratch directory")?;
    write_unresolvable_manifest(scratch.path())?;

    let child_env = cli_child_environment(std::env::vars_os().collect())?;
    let program = locate_or_build_cargo_bdd_command()?;
    let mut cmd = nested_cargo::filtered_command(
        Path::new(program.get_program()),
        &child_env,
        scratch.path(),
    );
    cmd.args(["steps"]);
    let output = cmd.output()?;
    let stderr = String::from_utf8_lossy(&output.stderr);

    assert!(
        !output.status.success(),
        "`cargo bdd steps` must fail when a dependency cannot be resolved offline; stderr was: \
         {stderr}"
    );
    assert!(
        stderr.contains("offline mode"),
        "the failure must come from offline resolution, which is the only thing that proves the \
         child ran offline; stderr was: {stderr}"
    );
    Ok(())
}
