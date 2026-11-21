//! Binary-level contract for the opt-in `CARGO_BDD_LOG` subscriber.
//!
//! The configuration loaders record no events of their own, so the span close
//! line is the only observable output of the opt-in. These child-process cases
//! prove a real binary renders it for both command labels, carries timing, and
//! leaks neither the configuration values nor the paths the loader consulted.

use std::io;

use rstest::rstest;

use super::{Invocation, IsolatedWorkspace, isolated_workspace};

/// One instrumented command and the arguments that keep it side-effect free.
struct LoggedCommand {
    /// Subcommand whose loader should open a configuration span.
    command: &'static str,
    /// Arguments chosen so the handler does no registry work.
    arguments: &'static [&'static str],
}

/// The `steps` loader, reached through its bypassed-step listing.
const STEPS: LoggedCommand = LoggedCommand {
    command: "steps",
    arguments: &["steps", "--skipped"],
};

/// The `skipped` loader, reached through its default listing.
const SKIPPED: LoggedCommand = LoggedCommand {
    command: "skipped",
    arguments: &["skipped"],
};

/// Assert one close line naming `command` and carrying both timing fields.
///
/// A macro so a failure reports the case's line rather than this body: the
/// two cases differ only in the command they name, and the reported location
/// is what separates them.
macro_rules! assert_timed_span_close {
    ($output:expr, $command:expr) => {{
        let output = $output;
        let command = $command;
        let stderr = String::from_utf8_lossy(&output.stderr);
        assert!(
            output.status.success(),
            "the logged run should succeed: {stderr}"
        );
        let span_lines = stderr
            .lines()
            .filter(|line| line.contains("cargo_bdd.configuration"))
            .collect::<Vec<_>>();
        assert_eq!(
            span_lines.len(),
            1,
            "each command loads configuration once: {stderr}"
        );
        let [line] = span_lines.as_slice() else {
            unreachable!("the length assertion above pins the line count");
        };
        assert!(
            line.contains(&format!("command=\"{command}\"")),
            "the span should name the {command} loader: {line}"
        );
        assert!(
            line.contains("close time.busy=") && line.contains("time.idle="),
            "the close line should carry timing: {line}"
        );
        assert!(
            !stderr.contains('\u{1b}'),
            "a captured log should contain no ANSI escapes: {stderr}"
        );
    }};
}

/// Assert the trace names no configuration value and no consulted path.
///
/// Returns its error instead of panicking for the one genuinely fallible step,
/// so a non-UTF-8 temporary path is reported as that rather than as a missing
/// leak. The assertions stay in the macro so their location is the call site.
macro_rules! assert_no_configuration_leak {
    ($output:expr, $workspace:expr) => {{ assert_no_configuration_leak!(@check $output, $workspace) }};
    (@check $output:expr, $workspace:expr) => {{
        let (output, workspace) = ($output, $workspace);
        let root = workspace
            .temporary_directory
            .path()
            .to_str()
            .ok_or_else(|| {
                io::Error::new(
                    io::ErrorKind::InvalidData,
                    "the test directory path is not UTF-8",
                )
            })?;
        let stderr = String::from_utf8_lossy(&output.stderr);
        for forbidden in [".cargo_bdd.toml", root] {
            assert!(
                !stderr.contains(forbidden),
                "the trace must not contain {forbidden}: {stderr}"
            );
        }
        Ok::<(), io::Error>(())
    }};
}

/// Opting in renders one timed span close line per configuration load.
#[rstest]
#[case::steps(STEPS)]
#[case::skipped(SKIPPED)]
fn opting_in_renders_one_timed_configuration_span(
    isolated_workspace: io::Result<IsolatedWorkspace>,
    #[case] logged: LoggedCommand,
) -> io::Result<()> {
    let workspace = isolated_workspace?;
    workspace.write_empty_package()?;
    let output = workspace.direct_with_environment(Invocation {
        arguments: logged.arguments,
        variables: &[("CARGO_BDD_LOG", "info")],
    })?;

    assert_timed_span_close!(&output, logged.command);
    assert_no_configuration_leak!(&output, &workspace)?;
    Ok(())
}
