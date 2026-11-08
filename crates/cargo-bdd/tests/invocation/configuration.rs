//! Subprocess coverage for configuration reaching the `skipped` and `steps`
//! handlers without CLI flags.
//!
//! Each case drives a real child process, so a resolver that ignored file or
//! environment values would fail here even though the merge unit tests still
//! passed. The precedence rows prove the tier order through the application
//! path rather than through the merge function alone.

use std::io;

use rstest::rstest;

use super::{Invocation, IsolatedWorkspace, assert_successful_json_listing, isolated_workspace};

/// One configuration source combination driven through a child process.
struct ConfigurationCase {
    /// Local `.cargo_bdd.toml` written before the run, if any.
    local_config: Option<&'static str>,
    /// Arguments passed to the binary.
    arguments: &'static [&'static str],
    /// Environment variables set in addition to the isolated defaults.
    variables: &'static [(&'static str, &'static str)],
}

/// A local file enables JSON output for `skipped`.
const SKIPPED_FILE: ConfigurationCase = ConfigurationCase {
    local_config: Some("[cmds.skipped]\njson = true\n"),
    arguments: &["skipped"],
    variables: &[],
};

/// The environment alone enables JSON output for `skipped`.
const SKIPPED_ENVIRONMENT: ConfigurationCase = ConfigurationCase {
    local_config: None,
    arguments: &["skipped"],
    variables: &[("CARGO_BDD_CMDS_SKIPPED_JSON", "true")],
};

/// A local file enables both `steps` settings without CLI flags.
const STEPS_FILE: ConfigurationCase = ConfigurationCase {
    local_config: Some("[cmds.steps]\nskipped = true\njson = true\n"),
    arguments: &["steps"],
    variables: &[],
};

/// The environment alone supplies both `steps` booleans.
const STEPS_ENVIRONMENT: ConfigurationCase = ConfigurationCase {
    local_config: None,
    arguments: &["steps"],
    variables: &[
        ("CARGO_BDD_CMDS_STEPS_SKIPPED", "true"),
        ("CARGO_BDD_CMDS_STEPS_JSON", "true"),
    ],
};

/// The environment tier overrides a file that disables JSON output.
const ENVIRONMENT_OVER_FILE: ConfigurationCase = ConfigurationCase {
    local_config: Some("[cmds.skipped]\njson = false\n"),
    arguments: &["skipped"],
    variables: &[("CARGO_BDD_CMDS_SKIPPED_JSON", "true")],
};

/// An explicit flag overrides an environment value that disables JSON output.
const FLAG_OVER_ENVIRONMENT: ConfigurationCase = ConfigurationCase {
    local_config: None,
    arguments: &["skipped", "--json"],
    variables: &[("CARGO_BDD_CMDS_SKIPPED_JSON", "false")],
};

/// The environment enables `steps.skipped` over a file that disables it.
const ENVIRONMENT_OVER_FILE_STEPS_SKIPPED: ConfigurationCase = ConfigurationCase {
    local_config: Some("[cmds.steps]\nskipped = false\njson = true\n"),
    arguments: &["steps"],
    variables: &[("CARGO_BDD_CMDS_STEPS_SKIPPED", "true")],
};

/// The environment enables `steps.json` over a file that disables it.
const ENVIRONMENT_OVER_FILE_STEPS_JSON: ConfigurationCase = ConfigurationCase {
    local_config: Some("[cmds.steps]\nskipped = true\njson = false\n"),
    arguments: &["steps"],
    variables: &[("CARGO_BDD_CMDS_STEPS_JSON", "true")],
};

/// An explicit `--json` overrides an environment value that disables it.
const FLAG_OVER_ENVIRONMENT_STEPS_JSON: ConfigurationCase = ConfigurationCase {
    local_config: None,
    arguments: &["steps", "--json"],
    variables: &[
        ("CARGO_BDD_CMDS_STEPS_SKIPPED", "true"),
        ("CARGO_BDD_CMDS_STEPS_JSON", "false"),
    ],
};

/// Every source tier reaches the handlers without an explicit flag.
///
/// The case names identify the tier each row exercises: `*_file` sets the
/// setting only in a local configuration file, `*_environment` only in the
/// environment, and the remaining rows assert the documented precedence of
/// environment over file and explicit flag over environment for each
/// configurable field. A row whose lower tier won would produce human-readable
/// output instead of the empty JSON array, so none of them can pass by
/// accident.
#[rstest]
#[case::skipped_file(SKIPPED_FILE)]
#[case::skipped_environment(SKIPPED_ENVIRONMENT)]
#[case::steps_file(STEPS_FILE)]
#[case::steps_environment(STEPS_ENVIRONMENT)]
#[case::environment_over_file(ENVIRONMENT_OVER_FILE)]
#[case::flag_over_environment(FLAG_OVER_ENVIRONMENT)]
#[case::steps_skipped_environment_over_file(ENVIRONMENT_OVER_FILE_STEPS_SKIPPED)]
#[case::steps_json_environment_over_file(ENVIRONMENT_OVER_FILE_STEPS_JSON)]
#[case::steps_json_flag_over_environment(FLAG_OVER_ENVIRONMENT_STEPS_JSON)]
fn configuration_reaches_the_handlers_from_every_source(
    isolated_workspace: io::Result<IsolatedWorkspace>,
    #[case] case: ConfigurationCase,
) -> io::Result<()> {
    let workspace = isolated_workspace?;
    workspace.write_empty_package()?;
    if let Some(contents) = case.local_config {
        workspace.write_local_config(contents)?;
    }

    let output = workspace.direct_with_environment(Invocation {
        arguments: case.arguments,
        variables: case.variables,
    })?;

    assert_successful_json_listing(&output);
    Ok(())
}
