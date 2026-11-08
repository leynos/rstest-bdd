//! File-tier configuration coverage against the minimal fixture crate.
//!
//! The parent module covers `skipped.reasons` through the environment tier.
//! This case proves the file tier reaches the same handler: the setting is
//! discovered from the platform configuration directory, with no CLI flag and
//! no `CARGO_BDD_` variable set. Both the XDG and the Windows configuration
//! roots are pointed at the same temporary directory, so one file serves
//! either platform's discovery order.

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
    root.create_dir_all("cargo_bdd")?;
    root.write("cargo_bdd/config.toml", "[cmds.skipped]\nreasons = true\n")?;
    let configuration_home = configuration_home
        .path()
        .to_str()
        .ok_or_else(|| eyre!("the temporary configuration path is not UTF-8"))?;

    let (status, stdout, stderr) = run_cargo_bdd_captured_with_env(
        &["skipped"],
        &[
            ("XDG_CONFIG_HOME", configuration_home),
            ("APPDATA", configuration_home),
        ],
    )?;

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
