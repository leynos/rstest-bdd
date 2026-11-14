//! The child environment shared by the CLI smoke tests.
//!
//! Every test that runs the `cargo-bdd` binary does so through a filtered
//! environment: the binary is spawned with a known map rather than the host's,
//! so a developer's own configuration cannot change what a case observes.
//! Building that map is one concern, and this module owns it so the tests that
//! assert on the result can stay in their own file.
//!
//! # Why this is a separate module
//!
//! The map is consumed by both the smoke tests in the parent module and the
//! `environment` module that asserts its shape. It is also large enough that
//! the reasoning behind each filter would take the test-target root past its
//! file-length budget if kept inline. The split is by cohesion, not by size
//! alone: the filters and their justifications belong together.

use std::{ffi::OsString, io};

use rstest_bdd_harness::nested_cargo;

use super::{discovery::empty_configuration_home_variables, isolation, workspace_root};

/// Apply the shared isolation policy with this CLI fixture's workspace target.
///
/// Only CLI smoke-test setup owns this fallback; explicit coverage and caller
/// target directories continue to take precedence.
///
/// Inherited `CARGO_BDD_*` variables are dropped before the shared policy runs.
/// The binary reads that prefix as a configuration tier, so a developer's own
/// export would otherwise outrank the file under test and make a file-tier case
/// assert against somebody else's setting. Cargo assigns no meaning to the
/// prefix, so nothing Cargo needs is lost.
///
/// The file tier needs the same treatment, and the prefix filter cannot give
/// it: discovery reads `$HOME/.cargo_bdd.toml`, the XDG bases and, on Windows,
/// the home dotfile, so an inherited `HOME` or `XDG_CONFIG_DIRS` lets a
/// developer's own file change what these smoke tests observe. Measured with a
/// `HOME` dotfile setting `cmds.skipped.json = true`,
/// `skipped_subcommand_includes_reasons_and_lines` fails on a run that
/// otherwise passes. Those roots are therefore pointed at an empty directory.
///
/// For example, `cli_child_environment(Vec::new())` selects the workspace
/// `target/`; a snapshot containing `CARGO_TARGET_DIR` retains that value.
///
/// The prefix test itself lives in [`isolation::carries_configuration_prefix`],
/// which owns the platform semantics behind it.
///
/// # Errors
///
/// Returns the error from preparing the empty discovery roots, and from
/// [`nested_cargo::env_from_vars`]: a relative target directory that cannot be
/// resolved against this process's working directory is reported rather than
/// handed to a child that would resolve it somewhere else.
///
/// Both propagate rather than panic so the cause survives: a redirect naming a
/// directory that was never created would otherwise surface as a
/// missing-configuration failure in whichever smoke test ran next. The `?`
/// returns before any environment is built, so a failure still cannot fall back
/// to the host's configuration roots — which is what this guard exists to
/// prevent.
pub(crate) fn cli_child_environment(
    mut vars: Vec<(OsString, OsString)>,
) -> io::Result<nested_cargo::NestedCargoEnv> {
    vars.retain(|(key, _)| !isolation::carries_configuration_prefix(key));
    // Built before extending so the helper can see what the caller asked for;
    // it passes a caller-supplied `CARGO_HOME` or `RUSTUP_HOME` through rather
    // than replacing it.
    let redirects = empty_configuration_home_variables(&vars)?;
    vars.extend(redirects);
    nested_cargo::env_from_vars(
        vars,
        Some(&workspace_root().join("target")),
        nested_cargo::ProfileDestination::Caller,
    )
}
