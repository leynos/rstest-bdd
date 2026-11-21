//! Property tests for source precedence, namespace isolation and validation.
//!
//! The example-based cases in the parent module pin one conflict sequence per
//! field. These properties compare every per-field combination of an absent,
//! explicitly false, or true value in each source tier against an independent
//! precedence model, while the other subcommand's file and environment
//! namespaces carry conflicting values that must not leak through.

use ortho_config::MapEnv;
use proptest::prelude::*;

use super::{BooleanOptions, ConfigFixture, validate_steps};

/// One source tier for a boolean setting: absent, explicitly false, or true.
fn optional_boolean() -> impl Strategy<Value = Option<bool>> { proptest::option::of(any::<bool>()) }

/// The documented precedence: defaults, then file, then environment, then CLI.
///
/// A CLI flag can only be present or absent, so presence always wins with
/// `true`; otherwise the environment value outranks the file value.
fn precedence(file: Option<bool>, environment: Option<bool>, cli: bool) -> bool {
    cli || environment.or(file).unwrap_or(false)
}

/// Render the `[cmds.<command>]` table for the settings that are present.
fn file_table(options: BooleanOptions, primary: Option<bool>, json: Option<bool>) -> String {
    let settings = [(false, primary), (true, json)]
        .into_iter()
        .filter_map(|(is_json, value)| {
            value.map(|value| format!("{} = {value}\n", options.option(is_json)))
        })
        .collect::<String>();
    format!("[cmds.{}]\n{settings}", options.command())
}

/// Pair each present setting with its `CARGO_BDD_` environment spelling.
fn environment_variables(
    options: BooleanOptions,
    primary: Option<bool>,
    json: Option<bool>,
) -> Vec<(String, &'static str)> {
    [(false, primary), (true, json)]
        .into_iter()
        .filter_map(|(is_json, value)| {
            value.map(|value| {
                (
                    options.environment_variable(is_json),
                    if value { "true" } else { "false" },
                )
            })
        })
        .collect()
}

/// Build the arguments and flags that are present on the command line.
fn cli_arguments(options: BooleanOptions, primary: bool, json: bool) -> Vec<&'static str> {
    let mut arguments = vec![options.command()];
    if primary {
        arguments.extend(options.cli_flags(false));
    }
    if json {
        arguments.extend(options.cli_flags(true));
    }
    arguments
}

/// The subcommand whose namespace must never reach the selected one.
const fn other_subcommand(options: BooleanOptions) -> BooleanOptions {
    match options {
        BooleanOptions::Steps => BooleanOptions::Skipped,
        BooleanOptions::Skipped => BooleanOptions::Steps,
    }
}

proptest! {
    /// Each merged field matches the precedence model, even when the other
    /// subcommand's file and environment namespaces carry conflicting values.
    #[test]
    fn merged_booleans_follow_the_source_precedence_model(
        options in prop_oneof![Just(BooleanOptions::Steps), Just(BooleanOptions::Skipped)],
        file_primary in optional_boolean(),
        file_json in optional_boolean(),
        environment_primary in optional_boolean(),
        environment_json in optional_boolean(),
        cli_primary in any::<bool>(),
        cli_json in any::<bool>(),
        other_file_primary in optional_boolean(),
        other_file_json in optional_boolean(),
        other_environment_primary in optional_boolean(),
        other_environment_json in optional_boolean(),
    ) {
        let fixture = ConfigFixture::new().expect("test setup should create the isolated fixture");
        let other = other_subcommand(options);
        let mut contents = file_table(options, file_primary, file_json);
        contents.push_str(&file_table(other, other_file_primary, other_file_json));
        fixture
            .write_local_config(&contents)
            .expect("test setup should write the local configuration file");

        let mut environment = MapEnv::new();
        let variables = environment_variables(options, environment_primary, environment_json)
            .into_iter()
            .chain(environment_variables(
                other,
                other_environment_primary,
                other_environment_json,
            ));
        for (key, value) in variables {
            environment = environment.with_var(key, value);
        }

        let merged = options
            .merge(&fixture, &cli_arguments(options, cli_primary, cli_json), environment)
            .expect("generated settings from valid sources should merge");
        prop_assert_eq!(
            merged,
            (
                precedence(file_primary, environment_primary, cli_primary),
                precedence(file_json, environment_json, cli_json),
            )
        );
    }

    /// Validation succeeds for exactly the merged combinations that do not
    /// request JSON without skipped steps.
    #[test]
    fn steps_validation_accepts_exactly_json_with_skipped(
        skipped in any::<bool>(),
        json in any::<bool>(),
    ) {
        let fixture = ConfigFixture::new().expect("test setup should create the isolated fixture");
        fixture
            .write_local_config(&file_table(BooleanOptions::Steps, Some(skipped), Some(json)))
            .expect("test setup should write the local configuration file");
        let merged = fixture
            .merge_steps(&["steps"], MapEnv::new())
            .expect("generated steps settings should merge");
        prop_assert_eq!((merged.skipped, merged.json), (skipped, json));
        prop_assert_eq!(validate_steps(&merged).is_ok(), !json || skipped);
    }
}
