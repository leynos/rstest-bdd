//! Source-injected tests for configuration precedence and failure handling.

use std::sync::Arc;

use camino::{Utf8Path, Utf8PathBuf};
use cap_std::{ambient_authority, fs::Dir};
use clap::{CommandFactory, FromArgMatches};
use ortho_config::{
    MapEnv,
    OrthoResult,
    SharedScanEnvSource,
    SubcommandCliMatches,
    SubcommandFileContext,
    load_and_merge_subcommand_for_with_matches_with_sources_at,
};
use rstest::{fixture, rstest};
use tempfile::TempDir;

use super::{SkippedConfig, StepsConfig, validate_steps};

struct ConfigFixture {
    _temporary_directory: TempDir,
    root: Dir,
    local_base: Utf8PathBuf,
    discovery: MapEnv,
}

impl ConfigFixture {
    fn new() -> Self {
        let Ok(temporary_directory) = tempfile::tempdir() else {
            panic!("test setup should create a temporary directory");
        };
        let Some(temporary_root) = Utf8Path::from_path(temporary_directory.path()) else {
            panic!("test directory should be representable as UTF-8");
        };
        let Ok(root) = Dir::open_ambient_dir(temporary_root.as_std_path(), ambient_authority())
        else {
            panic!("test setup should open the temporary directory capability");
        };
        for directory in [
            "home",
            "home/.config",
            "xdg-config-home",
            "xdg-config-home/cargo_bdd",
            "xdg-config-dirs",
            "local",
        ] {
            let Ok(()) = root.create_dir_all(directory) else {
                panic!("test setup should create isolated discovery roots");
            };
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

        Self {
            _temporary_directory: temporary_directory,
            root,
            local_base,
            discovery,
        }
    }

    fn write_local_config(&self, contents: &str) {
        let Ok(local) = self.root.open_dir("local") else {
            panic!("test setup should open the local configuration directory");
        };
        assert!(
            local.write(".cargo_bdd.toml", contents).is_ok(),
            "test setup should write the local configuration file"
        );
    }

    fn local_directory(&self) -> Dir {
        let Ok(local) = self.root.open_dir("local") else {
            panic!("test setup should open the local configuration directory");
        };
        local
    }

    fn merge_steps(&self, args: &[&str], environment: MapEnv) -> OrthoResult<StepsConfig> {
        let (config, matches) = parse_steps(args);
        self.merge(&config, &matches, environment)
    }

    fn merge_skipped(&self, args: &[&str], environment: MapEnv) -> OrthoResult<SkippedConfig> {
        let (config, matches) = parse_skipped(args);
        self.merge(&config, &matches, environment)
    }

    fn merge<T>(
        &self,
        config: &T,
        matches: &clap::ArgMatches,
        environment: MapEnv,
    ) -> OrthoResult<T>
    where
        T: ortho_config::OrthoConfig
            + serde::Serialize
            + serde::de::DeserializeOwned
            + Default
            + CommandFactory
            + ortho_config::CliValueExtractor,
    {
        let files = SubcommandFileContext::new(self.local_base.as_std_path(), &self.discovery);
        let merge_source: SharedScanEnvSource = Arc::new(environment);
        load_and_merge_subcommand_for_with_matches_with_sources_at(
            &SubcommandCliMatches::new(config, matches),
            files,
            merge_source,
        )
    }
}

#[rstest_bdd_test_macros::allow_fixture_expansion_lints]
#[fixture]
fn fixture() -> ConfigFixture { ConfigFixture::new() }

fn parse_steps(args: &[&str]) -> (StepsConfig, clap::ArgMatches) {
    let Ok(matches) = StepsConfig::command().try_get_matches_from(args) else {
        panic!("steps test arguments should parse");
    };
    let Ok(config) = StepsConfig::from_arg_matches(&matches) else {
        panic!("steps arguments should decode");
    };
    (config, matches)
}

fn parse_skipped(args: &[&str]) -> (SkippedConfig, clap::ArgMatches) {
    let Ok(matches) = SkippedConfig::command().try_get_matches_from(args) else {
        panic!("skipped test arguments should parse");
    };
    let Ok(config) = SkippedConfig::from_arg_matches(&matches) else {
        panic!("skipped arguments should decode");
    };
    (config, matches)
}

#[rstest]
fn steps_configuration_uses_defaults_file_environment_then_explicit_cli(fixture: ConfigFixture) {
    let defaults = fixture
        .merge_steps(&["steps"], MapEnv::new())
        .expect("missing optional files should preserve defaults");
    assert!(!defaults.skipped);
    assert!(!defaults.json);

    fixture.write_local_config("[cmds.steps]\nskipped = true\njson = true\n");
    let file = fixture
        .merge_steps(&["steps"], MapEnv::new())
        .expect("valid local settings should load");
    assert!(file.skipped);
    assert!(file.json);

    let environment = MapEnv::new()
        .with_var("CARGO_BDD_CMDS_STEPS_SKIPPED", "false")
        .with_var("CARGO_BDD_CMDS_STEPS_JSON", "false");
    let env = fixture
        .merge_steps(&["steps"], environment)
        .expect("injected environment values should load");
    assert!(!env.skipped);
    assert!(!env.json);

    let environment = MapEnv::new()
        .with_var("CARGO_BDD_CMDS_STEPS_SKIPPED", "false")
        .with_var("CARGO_BDD_CMDS_STEPS_JSON", "false");
    let cli = fixture
        .merge_steps(&["steps", "--skipped", "--json"], environment)
        .expect("explicit flags should override lower precedence tiers");
    assert!(cli.skipped);
    assert!(cli.json);
}

#[rstest]
fn skipped_configuration_uses_defaults_file_environment_then_explicit_cli(fixture: ConfigFixture) {
    let defaults = fixture
        .merge_skipped(&["skipped"], MapEnv::new())
        .expect("missing optional files should preserve defaults");
    assert!(!defaults.reasons);
    assert!(!defaults.json);

    fixture.write_local_config("[cmds.skipped]\nreasons = true\njson = true\n");
    let file = fixture
        .merge_skipped(&["skipped"], MapEnv::new())
        .expect("valid local settings should load");
    assert!(file.reasons);
    assert!(file.json);

    let environment = MapEnv::new()
        .with_var("CARGO_BDD_CMDS_SKIPPED_REASONS", "false")
        .with_var("CARGO_BDD_CMDS_SKIPPED_JSON", "false");
    let env = fixture
        .merge_skipped(&["skipped"], environment)
        .expect("injected environment values should load");
    assert!(!env.reasons);
    assert!(!env.json);

    let environment = MapEnv::new()
        .with_var("CARGO_BDD_CMDS_SKIPPED_REASONS", "false")
        .with_var("CARGO_BDD_CMDS_SKIPPED_JSON", "false");
    let cli = fixture
        .merge_skipped(&["skipped", "--reasons", "--json"], environment)
        .expect("explicit flags should override lower precedence tiers");
    assert!(cli.reasons);
    assert!(cli.json);
}

#[rstest]
fn settings_from_other_subcommand_namespaces_do_not_leak(fixture: ConfigFixture) {
    fixture.write_local_config("[cmds.skipped]\njson = true\n");
    let steps_from_other_file_namespace = fixture
        .merge_steps(&["steps"], MapEnv::new())
        .expect("unselected file namespaces should be ignored");
    fixture.write_local_config("[cmds.steps]\nskipped = true\n");
    let skipped_from_other_file_namespace = fixture
        .merge_skipped(&["skipped"], MapEnv::new())
        .expect("unselected file namespaces should be ignored");
    assert!(!steps_from_other_file_namespace.skipped && !steps_from_other_file_namespace.json);
    assert!(!skipped_from_other_file_namespace.reasons && !skipped_from_other_file_namespace.json);

    fixture.write_local_config("");
    let steps = fixture
        .merge_steps(
            &["steps"],
            MapEnv::new().with_var("CARGO_BDD_CMDS_SKIPPED_JSON", "true"),
        )
        .expect("unselected subcommand settings should be ignored");
    let skipped = fixture
        .merge_skipped(
            &["skipped"],
            MapEnv::new().with_var("CARGO_BDD_CMDS_STEPS_SKIPPED", "true"),
        )
        .expect("unselected subcommand settings should be ignored");
    assert!(!steps.skipped && !steps.json);
    assert!(!skipped.reasons && !skipped.json);
}

#[rstest]
fn unknown_keys_are_ignored_and_malformed_or_invalid_values_fail(fixture: ConfigFixture) {
    fixture.write_local_config("[cmds.steps]\nunknown = true\n");
    let unknown = fixture
        .merge_steps(&["steps"], MapEnv::new())
        .expect("OrthoConfig ignores unknown configuration fields");
    assert!(!unknown.skipped && !unknown.json);

    fixture.write_local_config("[cmds.steps\nskipped = true\n");
    assert!(
        fixture.merge_steps(&["steps"], MapEnv::new()).is_err(),
        "malformed TOML should be reported"
    );

    fixture.write_local_config("[cmds.steps]\nskipped = \"perhaps\"\n");
    assert!(
        fixture.merge_steps(&["steps"], MapEnv::new()).is_err(),
        "a value with the wrong type should be reported"
    );

    fixture.write_local_config("[cmds.steps]\n");
    assert!(
        fixture
            .merge_steps(
                &["steps"],
                MapEnv::new().with_var("CARGO_BDD_CMDS_STEPS_SKIPPED", "perhaps"),
            )
            .is_err(),
        "an environment value with the wrong type should be reported"
    );
}

#[rstest]
fn configuration_cannot_enable_steps_json_without_skipped_steps(fixture: ConfigFixture) {
    fixture.write_local_config("[cmds.steps]\njson = true\n");
    let config = fixture
        .merge_steps(&["steps"], MapEnv::new())
        .expect("individual TOML values should deserialize");
    let error =
        validate_steps(&config).expect_err("invalid cross-field settings should be rejected");
    let message = error.to_string();
    assert!(message.contains("cmds.steps.json"));
    assert!(message.contains("cmds.steps.skipped = true"));
}

#[cfg(unix)]
#[rstest]
fn unreadable_configuration_file_is_reported_when_permissions_are_enforced(fixture: ConfigFixture) {
    use cap_std::fs::{Permissions, PermissionsExt};

    fixture.write_local_config("[cmds.steps]\nskipped = true\n");
    let local = fixture.local_directory();
    local
        .set_permissions(".cargo_bdd.toml", Permissions::from_mode(0o000))
        .expect("test setup should remove file read permissions");
    if local.open(".cargo_bdd.toml").is_ok() {
        // Root can bypass Unix read permission bits, so this host cannot
        // exercise the loader's unreadable-file path through this fixture.
        return;
    }
    assert!(
        fixture.merge_steps(&["steps"], MapEnv::new()).is_err(),
        "the loader should report an unreadable discovered file"
    );
}
