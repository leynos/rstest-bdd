//! `skipped.reasons` coverage against the minimal fixture crate.
//!
//! These cases prove every tier reaches the handler — file, environment, and
//! the explicit flag — and that both documented precedence rules hold through
//! the application path: environment over file, and an explicit flag over the
//! environment. Every case also pins the absence of an application
//! diagnostic, so a working `reasons` setting cannot hide a failure reported
//! alongside it.
//!
//! Every discovery root the child can reach is redirected at the temporary
//! directory. That is not decoration: `ortho_config` merges *every* existing
//! candidate, so an inherited root lets a developer's own file load alongside
//! the fixture's — a malformed one fails the case, and a well-formed one can
//! satisfy a case that intended to prove the file tier. On Unix the merged
//! list is `$HOME/.cargo_bdd.toml`, every `$XDG_CONFIG_HOME` and
//! `XDG_CONFIG_DIRS` base, then the working-directory dotfile, so `HOME`,
//! `XDG_CONFIG_HOME` and `XDG_CONFIG_DIRS` are all overridden; an absent
//! `XDG_CONFIG_DIRS` would otherwise fall back to `/etc/xdg` and reintroduce
//! a host root. On Windows the home dotfile falls back from `HOME` to
//! `USERPROFILE`, and the native configuration directory comes from the
//! Windows API rather than `APPDATA`, so it cannot be redirected by any
//! variable and is left as the one documented residual. Cargo derives its own
//! home from the same variables, so the redirected run carries `CARGO_HOME`
//! and `RUSTUP_HOME` explicitly to keep the nested registry and toolchain
//! resolvable.

use std::env;

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
    variables.extend(
        case.variables
            .iter()
            .map(|(key, value)| ((*key).to_owned(), (*value).to_owned())),
    );

    let borrowed = variables
        .iter()
        .map(|(key, value)| (key.as_str(), value.as_str()))
        .collect::<Vec<_>>();
    let (status, stdout, stderr) = run_cargo_bdd_captured_with_env(case.arguments, &borrowed)?;

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
/// host. Application diagnostics keep stable prefixes at the start of a line,
/// so their absence is the property these cases can actually pin.
///
/// The probe is anchored rather than a substring search: the compiler wrapper
/// prints `<crate>: waiting for a compile slot`, and a crate named `thiserror`
/// would otherwise match `error:` and fail a case whose run was healthy.
fn assert_no_application_diagnostic(stderr: &str) {
    fn without_cause_index(line: &str) -> &str {
        let line = line.trim_start();
        let Some((index, rest)) = line.split_once(": ") else {
            return line;
        };
        if index.chars().all(|digit| digit.is_ascii_digit()) {
            rest
        } else {
            line
        }
    }

    for marker in ["failed to load configuration", "Error:", "error:"] {
        let found = stderr
            .lines()
            .map(without_cause_index)
            .any(|line| line.starts_with(marker));
        assert!(!found, "unexpected `{marker}` diagnostic: {stderr}");
    }
}

/// Build the variables that redirect configuration discovery.
///
/// The caller owns the temporary directory, so every path points at a location
/// valid for the whole run; the file itself is optional. `HOME` is redirected
/// on both platforms because both discover `$HOME/.cargo_bdd.toml`, and
/// `USERPROFILE` with it because Windows falls back to it. Unix also needs
/// `XDG_CONFIG_DIRS` set to a temporary directory rather than unset: unset
/// means `/etc/xdg`, which is a host root. The returned values are owned
/// `String`s so the caller can hand out references that outlive this function.
fn discovery_variables(root: &Dir, configuration_home: &str) -> Result<Vec<(String, String)>> {
    let mut variables = vec![
        ("HOME".to_owned(), configuration_home.to_owned()),
        ("USERPROFILE".to_owned(), configuration_home.to_owned()),
    ];
    if !cfg!(windows) {
        root.create_dir_all("cargo_bdd")?;
        variables.push(("XDG_CONFIG_HOME".to_owned(), configuration_home.to_owned()));
        variables.push(("XDG_CONFIG_DIRS".to_owned(), configuration_home.to_owned()));
    }
    variables.extend(cargo_home_variables());
    Ok(variables)
}

/// Carry the ambient Cargo and Rustup homes into the redirected child.
///
/// Cargo locates its registry through `CARGO_HOME`, defaulting to
/// `$HOME/.cargo`, so redirecting `HOME` alone would point the nested build at
/// an empty registry. `RUSTUP_HOME` is carried for the same reason: the child
/// resolves `cargo` through the ambient toolchain. A variable the developer
/// has already set wins over the `$HOME`-derived default, matching Cargo's own
/// precedence. `USERPROFILE` supplies the ambient home where `HOME` is absent,
/// as on Windows, because the child's own `USERPROFILE` is redirected.
fn cargo_home_variables() -> Vec<(String, String)> {
    let home = env::var("HOME").or_else(|_| env::var("USERPROFILE")).ok();
    [("CARGO_HOME", ".cargo"), ("RUSTUP_HOME", ".rustup")]
        .into_iter()
        .filter_map(|(key, directory)| {
            let derived = home.as_ref().map(|home| format!("{home}/{directory}"));
            env::var(key)
                .ok()
                .or(derived)
                .map(|value| (key.to_owned(), value))
        })
        .collect()
}

/// Write the platform's highest-precedence discovered configuration file.
fn write_discovered_file(root: &Dir, contents: &str) -> std::io::Result<()> {
    if cfg!(windows) {
        root.write(".cargo_bdd.toml", contents)
    } else {
        root.write("cargo_bdd/config.toml", contents)
    }
}
