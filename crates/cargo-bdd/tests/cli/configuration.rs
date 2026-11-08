//! File-tier configuration coverage against the minimal fixture crate.
//!
//! The parent module covers `skipped.reasons` through the environment tier.
//! This case proves the file tier reaches the same handler: the setting is
//! discovered from a file, with no CLI flag and no `CARGO_BDD_` variable set.
//!
//! The two platforms discover that file differently. Unix reads
//! `$XDG_CONFIG_HOME/cargo_bdd/config.toml`, so the temporary directory is
//! exposed as `XDG_CONFIG_HOME`. Windows reads `$HOME/.cargo_bdd.toml` before
//! the platform configuration directory, and that directory comes from the
//! Windows API rather than `APPDATA`, so only `HOME` can redirect it. `HOME`
//! is left alone on Unix because Cargo derives its own home from it.

use cap_std::{ambient_authority, fs::Dir};
use eyre::{Result, eyre};
use serial_test::serial;

use super::run_cargo_bdd_captured_with_env;

/// A file-configured `reasons` setting reaches the `skipped` handler.
#[test]
#[serial]
fn skipped_handler_uses_file_configured_reasons() -> Result<()> {
    let configuration_home = tempfile::tempdir()?;
    let root = Dir::open_ambient_dir(configuration_home.path(), ambient_authority())?;
    let configuration_home = configuration_home
        .path()
        .to_str()
        .ok_or_else(|| eyre!("the temporary configuration path is not UTF-8"))?;

    let variables: &[(&str, &str)] = if cfg!(windows) {
        root.write(".cargo_bdd.toml", "[cmds.skipped]\nreasons = true\n")?;
        &[("HOME", configuration_home)]
    } else {
        root.create_dir_all("cargo_bdd")?;
        root.write("cargo_bdd/config.toml", "[cmds.skipped]\nreasons = true\n")?;
        &[("XDG_CONFIG_HOME", configuration_home)]
    };

    let (status, stdout, stderr) = run_cargo_bdd_captured_with_env(&["skipped"], variables)?;

    assert!(
        status.success(),
        "the file-configured listing should succeed: {stderr}"
    );
    assert!(
        stdout.contains("tests/features/diagnostics.fixture:7"),
        "file-configured reasons should add feature locations: {stdout}"
    );
    assert!(
        stdout.contains("fixture skip message"),
        "file-configured reasons should add skip reasons: {stdout}"
    );
    Ok(())
}
