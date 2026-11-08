//! `skipped.reasons` coverage against the minimal fixture crate.
//!
//! These cases prove every tier reaches the handler — file, environment, and
//! the explicit flag — and that both documented precedence rules hold through
//! the application path: environment over file, and an explicit flag over the
//! environment. Every case also pins the absence of an application
//! diagnostic, so a working `reasons` setting cannot hide a failure reported
//! alongside it.
//!
//! The two platforms discover the file differently. Unix reads
//! `$XDG_CONFIG_HOME/cargo_bdd/config.toml`, so the temporary directory is
//! exposed as `XDG_CONFIG_HOME`. Windows reads `$HOME/.cargo_bdd.toml` before
//! the platform configuration directory, and that directory comes from the
//! Windows API rather than `APPDATA`, so only `HOME` can redirect it. `HOME`
//! is left alone on Unix because Cargo derives its own home from it.

use cap_std::{ambient_authority, fs::Dir};
use eyre::{Result, eyre};
use rstest::rstest;
use serial_test::serial;

use super::run_cargo_bdd_captured_with_env;

/// One `reasons` configuration combination driven through the real binary.
struct ReasonsCase {
    /// Discovered configuration file contents, or `None` to write no file.
    file: Option<&'static str>,
    /// Arguments passed to the binary.
    arguments: &'static [&'static str],
    /// Environment variables set in addition to the discovery redirect.
    variables: &'static [(&'static str, &'static str)],
}

/// A discovered file enables `reasons` with no flag and no variable.
const FILE_TIER: ReasonsCase = ReasonsCase {
    file: Some("[cmds.skipped]\nreasons = true\n"),
    arguments: &["skipped"],
    variables: &[],
};

/// The environment alone enables `reasons` with no file and no flag.
const ENVIRONMENT_TIER: ReasonsCase = ReasonsCase {
    file: None,
    arguments: &["skipped"],
    variables: &[("CARGO_BDD_CMDS_SKIPPED_REASONS", "true")],
};

/// The environment enables `reasons` over a file that disables it.
const ENVIRONMENT_OVER_FILE: ReasonsCase = ReasonsCase {
    file: Some("[cmds.skipped]\nreasons = false\n"),
    arguments: &["skipped"],
    variables: &[("CARGO_BDD_CMDS_SKIPPED_REASONS", "true")],
};

/// An explicit `--reasons` overrides an environment value that disables it.
const FLAG_OVER_ENVIRONMENT: ReasonsCase = ReasonsCase {
    file: None,
    arguments: &["skipped", "--reasons"],
    variables: &[("CARGO_BDD_CMDS_SKIPPED_REASONS", "false")],
};

/// Each configuration tier and precedence rule reaches the handler.
#[rstest]
#[case::file_tier(FILE_TIER)]
#[case::environment_tier(ENVIRONMENT_TIER)]
#[case::environment_over_file(ENVIRONMENT_OVER_FILE)]
#[case::flag_over_environment(FLAG_OVER_ENVIRONMENT)]
#[serial]
fn skipped_handler_uses_configured_reasons(#[case] case: ReasonsCase) -> Result<()> {
    let configuration_home = tempfile::tempdir()?;
    let root = Dir::open_ambient_dir(configuration_home.path(), ambient_authority())?;
    let configuration_home = configuration_home
        .path()
        .to_str()
        .ok_or_else(|| eyre!("the temporary configuration path is not UTF-8"))?;
    let mut variables = discovery_variables(&root, configuration_home)?;
    if let Some(contents) = case.file {
        write_discovered_file(&root, contents)
            .map_err(|error| eyre!("failed to write the discovered configuration: {error}"))?;
    }
    variables.extend_from_slice(case.variables);

    let (status, stdout, stderr) = run_cargo_bdd_captured_with_env(case.arguments, &variables)?;

    assert!(
        status.success(),
        "the configured listing should succeed: {stderr}"
    );
    assert_no_application_diagnostic(&stderr);
    assert!(
        stdout.contains("tests/features/diagnostics.fixture:7"),
        "configured reasons should add feature locations: {stdout}"
    );
    assert!(
        stdout.contains("fixture skip message"),
        "configured reasons should add skip reasons: {stdout}"
    );
    Ok(())
}

/// Assert the run emitted no diagnostic of its own.
///
/// These cases build a scenario fixture underneath `cargo bdd`, so the shared
/// compiler wrapper and the nested Cargo add their own progress lines to the
/// child's stderr. An empty-stream assertion would therefore depend on the
/// host. Application diagnostics keep stable prefixes, so their absence is the
/// property these cases can actually pin.
fn assert_no_application_diagnostic(stderr: &str) {
    for marker in ["failed to load configuration", "Error:", "error:"] {
        assert!(
            !stderr.contains(marker),
            "unexpected `{marker}` diagnostic: {stderr}"
        );
    }
}

/// Build the platform-specific variable that redirects configuration discovery.
///
/// The caller owns the temporary directory, so the variable points at a path
/// that is valid for the whole run; the file itself is optional.
fn discovery_variables<'a>(
    root: &Dir,
    configuration_home: &'a str,
) -> Result<Vec<(&'static str, &'a str)>> {
    if cfg!(windows) {
        return Ok(vec![("HOME", configuration_home)]);
    }
    root.create_dir_all("cargo_bdd")?;
    Ok(vec![("XDG_CONFIG_HOME", configuration_home)])
}

/// Write the platform's highest-precedence discovered configuration file.
fn write_discovered_file(root: &Dir, contents: &str) -> std::io::Result<()> {
    if cfg!(windows) {
        root.write(".cargo_bdd.toml", contents)
    } else {
        root.write("cargo_bdd/config.toml", contents)
    }
}
