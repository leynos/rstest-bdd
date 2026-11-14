//! Shared fixture and option table for the configuration precedence tests.
//!
//! The fixture isolates every discovery root (home, XDG config home, XDG
//! config dirs and the local base) inside one temporary directory, so a
//! developer's real configuration cannot influence a merge. The option table
//! names each configurable boolean once, so the precedence test can drive all
//! four settings through the same code path.
//!
//! The merges below call the production resolvers with these isolated sources
//! rather than repeating the `ortho_config` call, so a regression in the
//! resolvers fails these tests.

use std::sync::Arc;

use camino::{Utf8Path, Utf8PathBuf};
use cap_std::{ambient_authority, fs::Dir};
use clap::{CommandFactory, FromArgMatches};
use eyre::{WrapErr, eyre};
use ortho_config::MapEnv;
use rstest::fixture;
use tempfile::TempDir;

use super::super::{
    ConfigSources,
    SkippedConfig,
    SkippedSettings,
    StepsConfig,
    merge_steps,
    resolve_skipped,
};

/// Isolated discovery roots plus the injected environment for one merge.
pub(super) struct ConfigFixture {
    /// Owns the temporary directory for as long as the fixture lives.
    _temporary_directory: TempDir,
    /// Capability scoped to the temporary directory root.
    root: Dir,
    /// Directory the loader treats as the local configuration base.
    local_base: Utf8PathBuf,
    /// Environment values the loader sees instead of the process environment.
    discovery: MapEnv,
}

/// The configurable boolean a precedence case exercises.
#[derive(Debug, Clone, Copy)]
pub(super) enum BooleanOptions {
    /// The `steps` subcommand's settings.
    Steps,
    /// The `skipped` subcommand's settings.
    Skipped,
}

impl BooleanOptions {
    /// Name of the subcommand that owns the setting.
    pub(super) const fn command(self) -> &'static str {
        match self {
            Self::Steps => "steps",
            Self::Skipped => "skipped",
        }
    }

    /// Configuration key for the setting, given whether it is the JSON one.
    pub(super) const fn option(self, is_json: bool) -> &'static str {
        if is_json {
            "json"
        } else {
            match self {
                Self::Steps => "skipped",
                Self::Skipped => "reasons",
            }
        }
    }

    /// CLI flags that set the setting explicitly.
    pub(super) const fn cli_flags(self, is_json: bool) -> &'static [&'static str] {
        match (self, is_json) {
            (Self::Steps | Self::Skipped, true) => &["--json"],
            (Self::Steps, false) => &["--skipped"],
            (Self::Skipped, false) => &["--reasons"],
        }
    }

    /// Environment variable that carries the setting.
    pub(super) fn environment_variable(self, is_json: bool) -> String {
        format!(
            "CARGO_BDD_CMDS_{}_{}",
            self.command().to_ascii_uppercase(),
            self.option(is_json).to_ascii_uppercase()
        )
    }

    /// Expected `(non-JSON, JSON)` pair after merging with `enabled`.
    pub(super) const fn expected_values(self, is_json: bool, enabled: bool) -> (bool, bool) {
        match (self, is_json) {
            (Self::Steps, true) => (true, enabled),
            (Self::Steps | Self::Skipped, false) => (enabled, false),
            (Self::Skipped, true) => (false, enabled),
        }
    }

    /// Merge the subcommand's settings and return its two booleans.
    pub(super) fn merge(
        self,
        fixture: &ConfigFixture,
        args: &[&str],
        environment: MapEnv,
    ) -> eyre::Result<(bool, bool)> {
        match self {
            Self::Steps => fixture
                .merge_steps(args, environment)
                .map(|config| (config.skipped, config.json)),
            Self::Skipped => fixture
                .merge_skipped(args, environment)
                .map(|config| (config.reasons, config.json)),
        }
    }
}

impl ConfigFixture {
    /// Create the isolated directory tree and discovery environment.
    ///
    /// Fallible so a temporary-directory, permission or write failure keeps
    /// its original diagnostic instead of becoming a fixed panic message.
    pub(super) fn new() -> eyre::Result<Self> {
        let temporary_directory =
            tempfile::tempdir().wrap_err("test setup should create a temporary directory")?;
        let temporary_root = Utf8Path::from_path(temporary_directory.path())
            .ok_or_else(|| eyre!("test directory should be representable as UTF-8"))?;
        let root = Dir::open_ambient_dir(temporary_root.as_std_path(), ambient_authority())
            .wrap_err("test setup should open the temporary directory capability")?;
        for directory in [
            "home",
            "home/.config",
            "xdg-config-home",
            "xdg-config-home/cargo_bdd",
            "xdg-config-dirs",
            "local",
        ] {
            root.create_dir_all(directory)
                .wrap_err("test setup should create isolated discovery roots")?;
        }
        let home = temporary_root.join("home");
        let xdg_config_home = temporary_root.join("xdg-config-home");
        let xdg_config_dirs = temporary_root.join("xdg-config-dirs");
        let local_base = temporary_root.join("local");
        let discovery = MapEnv::new()
            .with_var("HOME", &home)
            .with_var("USERPROFILE", &home)
            .with_var("XDG_CONFIG_HOME", &xdg_config_home)
            .with_var("XDG_CONFIG_DIRS", &xdg_config_dirs)
            .with_var("APPDATA", &xdg_config_home);

        Ok(Self {
            _temporary_directory: temporary_directory,
            root,
            local_base,
            discovery,
        })
    }

    /// Write the local `.cargo_bdd.toml` the loader discovers first.
    pub(super) fn write_local_config(&self, contents: &str) -> eyre::Result<()> {
        let local = self
            .root
            .open_dir("local")
            .wrap_err("test setup should open the local configuration directory")?;
        local
            .write(".cargo_bdd.toml", contents)
            .wrap_err("test setup should write the local configuration file")
    }

    /// Open the local configuration directory for permission experiments.
    #[cfg(unix)]
    pub(super) fn local_directory(&self) -> eyre::Result<Dir> {
        self.root
            .open_dir("local")
            .wrap_err("test setup should open the local configuration directory")
    }

    /// Merge `steps` settings through the production resolver.
    pub(super) fn merge_steps(
        &self,
        args: &[&str],
        environment: MapEnv,
    ) -> eyre::Result<StepsConfig> {
        let (config, matches) = parse_steps(args)?;
        merge_steps(&config, &matches, &self.sources(environment))
    }

    /// Merge `skipped` settings through the production resolver.
    pub(super) fn merge_skipped(
        &self,
        args: &[&str],
        environment: MapEnv,
    ) -> eyre::Result<SkippedSettings> {
        let (config, matches) = parse_skipped(args)?;
        resolve_skipped(&config, &matches, &self.sources(environment))
    }

    /// Build the isolated sources one merge reads.
    pub(super) fn sources(&self, environment: MapEnv) -> ConfigSources {
        ConfigSources::isolated(
            self.local_base.clone(),
            self.discovery.clone(),
            Arc::new(environment),
        )
    }
}

/// Create the isolated configuration fixture.
#[rstest_bdd_test_macros::allow_fixture_expansion_lints]
#[fixture]
pub(super) fn fixture() -> eyre::Result<ConfigFixture> { ConfigFixture::new() }

/// Parse `steps` arguments into a config value and its matches.
fn parse_steps(args: &[&str]) -> eyre::Result<(StepsConfig, clap::ArgMatches)> {
    let matches = StepsConfig::command()
        .try_get_matches_from(args)
        .wrap_err("steps test arguments should parse")?;
    let config =
        StepsConfig::from_arg_matches(&matches).wrap_err("steps arguments should decode")?;
    Ok((config, matches))
}

/// Parse `skipped` arguments into a config value and its matches.
fn parse_skipped(args: &[&str]) -> eyre::Result<(SkippedConfig, clap::ArgMatches)> {
    let matches = SkippedConfig::command()
        .try_get_matches_from(args)
        .wrap_err("skipped test arguments should parse")?;
    let config =
        SkippedConfig::from_arg_matches(&matches).wrap_err("skipped arguments should decode")?;
    Ok((config, matches))
}
