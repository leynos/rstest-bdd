//! Source-injected tests for configuration precedence and failure handling.
//!
//! Each test supplies isolated discovery roots and an injected environment
//! through the `fixture` module, then asserts what the real `ortho_config`
//! loader produces. Keeping the fixture separate leaves this file to state the
//! precedence and failure contracts themselves.

use ortho_config::MapEnv;
use rstest::rstest;
use support::{BooleanOptions, ConfigFixture, fixture};

use super::{StepsSettings, validate_steps};

// The fixture's module is named `support` so the `#[fixture]` function it
// exports can keep the `fixture` name that the `#[rstest]` parameters look up.
#[path = "tests/fixture.rs"]
mod support;

#[path = "tests/observability.rs"]
mod observability;

#[path = "tests/property.rs"]
mod property;

/// Defaults yield false, a file enables the setting, an environment value
/// overrides the file, and an explicit flag overrides the environment.
#[rstest]
#[case::steps_skipped(BooleanOptions::Steps, false)]
#[case::steps_json(BooleanOptions::Steps, true)]
#[case::skipped_reasons(BooleanOptions::Skipped, false)]
#[case::skipped_json(BooleanOptions::Skipped, true)]
fn subcommand_configuration_uses_defaults_file_environment_then_cli(
    fixture: eyre::Result<ConfigFixture>,
    #[case] options: BooleanOptions,
    #[case] is_json: bool,
) {
    let fixture = fixture.expect("test setup should create the isolated fixture");
    let command = options.command();
    let default_args = [command];
    let defaults = options
        .merge(&fixture, &default_args, MapEnv::new())
        .expect("missing optional files should preserve defaults");
    assert_eq!(defaults, (false, false));

    let setting = options.option(is_json);
    let support_setting = if matches!(options, BooleanOptions::Steps) && is_json {
        "skipped = true\n"
    } else {
        ""
    };
    let file_config = format!("[cmds.{command}]\n{support_setting}{setting} = true\n");
    fixture
        .write_local_config(&file_config)
        .expect("test setup should write the local configuration file");
    let file = options
        .merge(&fixture, &default_args, MapEnv::new())
        .expect("valid local settings should load");
    assert_eq!(file, options.expected_values(is_json, true));

    let environment = MapEnv::new().with_var(options.environment_variable(is_json), "false");
    let env = options
        .merge(&fixture, &default_args, environment)
        .expect("injected environment values should load");
    assert_eq!(env, options.expected_values(is_json, false));

    let cli_args = [command]
        .into_iter()
        .chain(options.cli_flags(is_json).iter().copied())
        .collect::<Vec<_>>();
    let environment = MapEnv::new().with_var(options.environment_variable(is_json), "false");
    let cli = options
        .merge(&fixture, &cli_args, environment)
        .expect("explicit flags should override lower precedence tiers");
    assert_eq!(cli, options.expected_values(is_json, true));
}

/// A subcommand ignores settings addressed to a different subcommand.
#[rstest]
fn settings_from_other_subcommand_namespaces_do_not_leak(fixture: eyre::Result<ConfigFixture>) {
    let fixture = fixture.expect("test setup should create the isolated fixture");
    fixture
        .write_local_config("[cmds.skipped]\njson = true\n")
        .expect("test setup should write the local configuration file");
    let steps_from_other_file_namespace = fixture
        .merge_steps(&["steps"], MapEnv::new())
        .expect("unselected file namespaces should be ignored");
    fixture
        .write_local_config("[cmds.steps]\nskipped = true\n")
        .expect("test setup should write the local configuration file");
    let skipped_from_other_file_namespace = fixture
        .merge_skipped(&["skipped"], MapEnv::new())
        .expect("unselected file namespaces should be ignored");
    assert!(!steps_from_other_file_namespace.skipped && !steps_from_other_file_namespace.json);
    assert!(!skipped_from_other_file_namespace.reasons && !skipped_from_other_file_namespace.json);

    fixture
        .write_local_config("")
        .expect("test setup should write the local configuration file");
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

/// Unknown keys are ignored, while malformed or mistyped values are reported.
#[rstest]
fn unknown_keys_are_ignored_and_malformed_or_invalid_values_fail(
    fixture: eyre::Result<ConfigFixture>,
) {
    let fixture = fixture.expect("test setup should create the isolated fixture");
    fixture
        .write_local_config("[cmds.steps]\nunknown = true\n")
        .expect("test setup should write the local configuration file");
    let unknown = fixture
        .merge_steps(&["steps"], MapEnv::new())
        .expect("OrthoConfig ignores unknown configuration fields");
    assert!(!unknown.skipped && !unknown.json);

    fixture
        .write_local_config("[cmds.steps\nskipped = true\n")
        .expect("test setup should write the local configuration file");
    assert!(
        fixture.merge_steps(&["steps"], MapEnv::new()).is_err(),
        "malformed TOML should be reported"
    );

    fixture
        .write_local_config("[cmds.steps]\nskipped = \"perhaps\"\n")
        .expect("test setup should write the local configuration file");
    assert!(
        fixture.merge_steps(&["steps"], MapEnv::new()).is_err(),
        "a value with the wrong type should be reported"
    );

    fixture
        .write_local_config("[cmds.steps]\n")
        .expect("test setup should write the local configuration file");
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

/// The cross-field rule rejects JSON output that no merged source enables.
#[rstest]
fn configuration_cannot_enable_steps_json_without_skipped_steps(
    fixture: eyre::Result<ConfigFixture>,
) {
    let fixture = fixture.expect("test setup should create the isolated fixture");
    fixture
        .write_local_config("[cmds.steps]\njson = true\n")
        .expect("test setup should write the local configuration file");
    let config = fixture
        .merge_steps(&["steps"], MapEnv::new())
        .expect("individual TOML values should deserialize");
    let error =
        validate_steps(&config).expect_err("invalid cross-field settings should be rejected");
    let message = error.to_string();
    assert!(message.contains("cmds.steps.json"));
    assert!(message.contains("cmds.steps.skipped = true"));
}

/// An explicit JSON flag borrows `skipped` from whichever source supplies it.
#[rstest]
#[case::file(true)]
#[case::environment(false)]
fn explicit_steps_json_uses_skipped_from_merged_sources(
    fixture: eyre::Result<ConfigFixture>,
    #[case] use_file: bool,
) {
    let fixture = fixture.expect("test setup should create the isolated fixture");
    let environment = if use_file {
        fixture
            .write_local_config("[cmds.steps]\nskipped = true\n")
            .expect("test setup should write the local configuration file");
        MapEnv::new()
    } else {
        fixture
            .write_local_config("")
            .expect("test setup should write the local configuration file");
        MapEnv::new().with_var("CARGO_BDD_CMDS_STEPS_SKIPPED", "true")
    };
    let merged = fixture
        .merge_steps(&["steps", "--json"], environment)
        .expect("the explicit JSON flag and source setting should merge");

    assert_eq!(
        validate_steps(&merged).expect("merged skipped setting should satisfy JSON mode"),
        StepsSettings {
            skipped: true,
            json: true,
        }
    );
}

/// An unreadable discovered file is reported rather than skipped.
#[cfg(unix)]
#[rstest]
fn unreadable_configuration_file_is_reported_when_permissions_are_enforced(
    fixture: eyre::Result<ConfigFixture>,
) {
    use cap_std::fs::{Permissions, PermissionsExt};

    let fixture = fixture.expect("test setup should create the isolated fixture");
    fixture
        .write_local_config("[cmds.steps]\nskipped = true\n")
        .expect("test setup should write the local configuration file");
    let local = fixture
        .local_directory()
        .expect("test setup should open the local configuration directory");
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
