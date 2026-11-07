//! Subprocess coverage for configuration reaching the `skipped` and `steps`
//! handlers without CLI flags.
//!
//! The parent module's cases configure `steps.skipped` only. These cases drive
//! the `skipped` command and the `steps.json` setting through a real child
//! process, so a resolver that ignored file or environment values would fail
//! here even though the merge unit tests still passed.

use std::io;

use rstest::rstest;

use super::{IsolatedWorkspace, assert_successful_json_listing, isolated_workspace};

/// A local file that enables JSON output for the `skipped` command.
#[rstest]
fn skipped_handler_uses_file_configuration(
    isolated_workspace: io::Result<IsolatedWorkspace>,
) -> io::Result<()> {
    let workspace = isolated_workspace?;
    workspace.write_empty_package()?;
    workspace.write_local_config("[cmds.skipped]\njson = true\n")?;

    let output = workspace.direct(&["skipped"])?;

    assert_successful_json_listing(&output);
    Ok(())
}

/// An environment variable that enables JSON output for the `skipped` command.
#[rstest]
fn skipped_handler_uses_environment_configuration(
    isolated_workspace: io::Result<IsolatedWorkspace>,
) -> io::Result<()> {
    let workspace = isolated_workspace?;
    workspace.write_empty_package()?;

    let output = workspace.direct_with_env(&["skipped"], "CARGO_BDD_CMDS_SKIPPED_JSON", "true")?;

    assert_successful_json_listing(&output);
    Ok(())
}

/// A local file that enables JSON output for `steps` without CLI flags.
#[rstest]
fn steps_handler_uses_configured_json_without_cli_flags(
    isolated_workspace: io::Result<IsolatedWorkspace>,
) -> io::Result<()> {
    let workspace = isolated_workspace?;
    workspace.write_empty_package()?;
    workspace.write_local_config("[cmds.steps]\nskipped = true\njson = true\n")?;

    let output = workspace.direct(&["steps"])?;

    assert_successful_json_listing(&output);
    Ok(())
}
