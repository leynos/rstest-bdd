//! Binary-level contract for direct and Cargo-dispatched CLI entry points.
//!
//! The isolated workspace fixture lives in the `workspace` submodule so this
//! file stays focused on the observable contract: which invocation forms reach
//! a handler, and which bytes and exit statuses each failure produces.

use std::{io, process::Output};

use rstest::rstest;

#[path = "invocation/configuration.rs"]
mod configuration;
#[path = "invocation/observability.rs"]
mod observability;
#[path = "invocation/workspace.rs"]
mod workspace;

use workspace::{Invocation, IsolatedWorkspace, isolated_workspace};

/// Every invocation form renders the same `steps --help` usage text.
#[rstest]
fn flat_injected_and_real_cargo_forms_share_the_same_command_help(
    isolated_workspace: io::Result<IsolatedWorkspace>,
) -> io::Result<()> {
    let workspace = isolated_workspace?;
    let flat = workspace.direct(&["steps", "--help"])?;
    let injected = workspace.direct(&["bdd", "steps", "--help"])?;
    let cargo = workspace.cargo_dispatch(&["steps", "--help"])?;

    let flat_usage = if cfg!(windows) {
        "Usage: cargo-bdd.exe steps"
    } else {
        "Usage: cargo-bdd steps"
    };
    assert_successful_help(&flat, flat_usage);
    assert_successful_help(&injected, "Usage: cargo bdd steps");
    assert_matching_streams(&injected, &cargo);
    Ok(())
}

/// File configuration reaches the handler through every invocation form.
#[rstest]
fn all_invocation_forms_reach_the_steps_handler_with_file_configuration(
    isolated_workspace: io::Result<IsolatedWorkspace>,
) -> io::Result<()> {
    let workspace = isolated_workspace?;
    workspace.write_empty_package()?;
    workspace.write_local_config("[cmds.steps]\nskipped = true\n")?;

    let flat = workspace.direct(&["steps", "--json"])?;
    let injected = workspace.direct(&["bdd", "steps", "--json"])?;
    let cargo = workspace.cargo_dispatch(&["steps", "--json"])?;

    for output in [&flat, &injected, &cargo] {
        assert_successful_json_listing(output);
    }
    Ok(())
}

/// An explicit flag combines with an environment-provided `skipped` setting.
#[rstest]
fn explicit_json_uses_environment_configuration_in_a_child_process(
    isolated_workspace: io::Result<IsolatedWorkspace>,
) -> io::Result<()> {
    let workspace = isolated_workspace?;
    workspace.write_empty_package()?;
    let output = workspace.direct_with_environment(Invocation {
        arguments: &["steps", "--json"],
        variables: &[("CARGO_BDD_CMDS_STEPS_SKIPPED", "true")],
    })?;

    assert_successful_json_listing(&output);
    Ok(())
}

/// Without configuration, `steps --json` keeps Clap's original failure bytes.
#[rstest]
fn flat_steps_json_without_configuration_keeps_the_original_parse_failure(
    isolated_workspace: io::Result<IsolatedWorkspace>,
) -> io::Result<()> {
    let workspace = isolated_workspace?;
    let flat = workspace.direct(&["steps", "--json"])?;
    let injected = workspace.direct(&["bdd", "steps", "--json"])?;
    let cargo = workspace.cargo_dispatch(&["steps", "--json"])?;
    let expected_flat_stderr = concat!(
        "error: the following required arguments were not provided:\n",
        "  --skipped\n\n",
        "Usage: cargo-bdd steps --skipped --json\n\n",
        "For more information, try '--help'.\n"
    );

    assert_missing_skipped_error(&flat, expected_flat_stderr);
    assert_matching_parse_failures(&injected, &cargo);
    Ok(())
}

/// An unknown command inside the injected wrapper stays a parse error.
#[rstest]
fn injected_wrapper_keeps_unknown_inner_commands_as_parse_errors(
    isolated_workspace: io::Result<IsolatedWorkspace>,
) -> io::Result<()> {
    let workspace = isolated_workspace?;
    let output = workspace.direct(&["bdd", "unknown-command"])?;

    assert_eq!(output.status.code(), Some(2));
    assert!(String::from_utf8_lossy(&output.stderr).contains("unknown-command"));
    Ok(())
}

/// Malformed or mistyped configuration exits one with a diagnostic on stderr.
#[rstest]
#[case::malformed_toml("[cmds.steps\nskipped = true\n")]
#[case::invalid_value_type("[cmds.steps]\nskipped = \"perhaps\"\n")]
fn configuration_file_errors_exit_one_on_stderr(
    isolated_workspace: io::Result<IsolatedWorkspace>,
    #[case] contents: &str,
) -> io::Result<()> {
    let workspace = isolated_workspace?;
    workspace.write_local_config(contents)?;

    let output = workspace.direct(&["steps"])?;

    assert_configuration_error(&output);
    Ok(())
}

/// An unreadable configuration file is reported rather than ignored.
#[rstest]
fn unreadable_local_configuration_exits_one_on_stderr(
    isolated_workspace: io::Result<IsolatedWorkspace>,
) -> io::Result<()> {
    let workspace = isolated_workspace?;
    workspace.write_local_config("[cmds.steps]\nskipped = true\n")?;
    let output = {
        #[cfg(unix)]
        {
            use cap_std::fs::{Permissions, PermissionsExt};

            let working_directory = workspace.root.open_dir("work")?;
            working_directory.set_permissions(".cargo_bdd.toml", Permissions::from_mode(0o000))?;
            let current_user_can_read = working_directory.open(".cargo_bdd.toml").is_ok();
            #[cfg(target_os = "linux")]
            {
                if current_user_can_read {
                    workspace.direct_as_unprivileged(&["steps"])?
                } else {
                    workspace.direct(&["steps"])?
                }
            }
            #[cfg(not(target_os = "linux"))]
            {
                if current_user_can_read {
                    // Some privileged Unix runners can bypass file permission bits.
                    return Ok(());
                }
                workspace.direct(&["steps"])?
            }
        }
        #[cfg(windows)]
        {
            use cap_std::fs::{OpenOptions, OpenOptionsExt};

            let working_directory = workspace.root.open_dir("work")?;
            let mut options = OpenOptions::new();
            options.write(true).share_mode(0);
            let _exclusive_file = working_directory.open_with(".cargo_bdd.toml", &options)?;
            workspace.direct(&["steps"])?
        }
        #[cfg(not(any(unix, windows)))]
        {
            return Ok(());
        }
    };

    assert_configuration_error(&output);
    Ok(())
}

/// Assert the exit status, empty stdout and stderr prefix of a load failure.
fn assert_configuration_error(output: &Output) {
    assert_eq!(output.status.code(), Some(1));
    assert_eq!(output.stdout, [] as [u8; 0]);
    let stderr = String::from_utf8_lossy(&output.stderr);
    assert!(stderr.contains("failed to load configuration for `steps`"));
}

/// Assert a successful help invocation that printed `usage` and no stderr.
fn assert_successful_help(output: &Output, usage: &str) {
    assert!(output.status.success(), "help invocation should succeed");
    assert!(String::from_utf8_lossy(&output.stdout).contains(usage));
    assert_eq!(output.stderr, [] as [u8; 0]);
}

/// Assert a successful listing that emitted exactly an empty JSON array.
fn assert_successful_json_listing(output: &Output) {
    assert!(output.status.success(), "JSON listing should succeed");
    assert_eq!(
        (output.stdout.as_slice(), output.stderr.as_slice()),
        (b"[]\n" as &[u8], b"" as &[u8])
    );
}

/// Assert two invocations produced identical stdout and stderr.
fn assert_matching_streams(actual: &Output, expected: &Output) {
    assert_eq!(
        (actual.stdout.as_slice(), actual.stderr.as_slice()),
        (expected.stdout.as_slice(), expected.stderr.as_slice())
    );
}

/// Assert the exact exit status, empty stdout and stderr of a parse failure.
fn assert_missing_skipped_error(output: &Output, expected_stderr: &str) {
    assert_eq!(
        (
            output.status.code(),
            output.stdout.as_slice(),
            output.stderr.as_slice()
        ),
        (Some(2), b"" as &[u8], expected_stderr.as_bytes())
    );
}

/// Assert the injected and Cargo-dispatched forms fail identically.
fn assert_matching_parse_failures(injected: &Output, cargo: &Output) {
    assert_eq!(
        (injected.status.code(), cargo.status.code()),
        (Some(2), Some(2))
    );
    assert_eq!(
        (injected.stdout.as_slice(), injected.stderr.as_slice()),
        (cargo.stdout.as_slice(), cargo.stderr.as_slice())
    );
    assert!(String::from_utf8_lossy(&cargo.stderr).contains("--skipped"));
}
