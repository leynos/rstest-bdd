//! Direct and Cargo-dispatched argument parsing for the `cargo-bdd` binary.
//!
//! This module owns the parser wrapper for the application binary and preserves
//! the prior Clap error for a `steps --json` invocation that has no effective
//! `skipped` setting. It is private to `cargo-bdd` and is called by `cli::run`.
//!
//! Neither parser nor settings resolution terminates the process here. Both
//! return [`CliError`], and `main` prints a Clap failure with Clap's own
//! renderer and exit status, so the bytes on each stream are unchanged while
//! callers and tests can inspect the failure in process.

use std::ffi::OsString;

use clap::{ArgMatches, CommandFactory, FromArgMatches};
use ortho_config::cargo::external_subcommand;

use super::Cli;

/// Failure from parsing arguments or resolving subcommand settings.
#[derive(Debug)]
pub(crate) enum CliError {
    /// A Clap failure the application boundary must print and exit with.
    Clap(clap::Error),
    /// Any other failure, reported through `eyre` at the application boundary.
    Report(eyre::Report),
}

impl From<eyre::Report> for CliError {
    /// Treat any `eyre` failure as a report for the application boundary.
    fn from(report: eyre::Report) -> Self { Self::Report(report) }
}

/// Entry shape used to select the parser and its compatibility output.
#[derive(Clone, Copy)]
pub(super) enum InvocationForm {
    /// The binary was called directly as `cargo-bdd <subcommand>`.
    Direct,
    /// Cargo injected `bdd` before the command name.
    CargoDispatched,
}

/// Parse either the direct binary form or Cargo's injected `bdd` form.
pub(super) fn parse_command_line<I>(
    arguments: I,
) -> Result<(Cli, ArgMatches, InvocationForm), CliError>
where
    I: IntoIterator<Item = OsString>,
{
    let arguments = arguments.into_iter().collect::<Vec<_>>();
    if is_cargo_dispatched(&arguments) {
        parse_cargo_dispatched(arguments)
    } else {
        parse_direct(arguments)
    }
}

/// Report whether Cargo injected its `bdd` subcommand name.
fn is_cargo_dispatched(arguments: &[OsString]) -> bool {
    arguments.get(1).is_some_and(|argument| argument == "bdd")
}

/// Parse the `cargo bdd <command>` form Cargo produces.
fn parse_cargo_dispatched(
    arguments: Vec<OsString>,
) -> Result<(Cli, ArgMatches, InvocationForm), CliError> {
    let wrapped_matches = external_subcommand("cargo-bdd", "bdd", Cli::command())
        .try_get_matches_from(arguments)
        .map_err(CliError::Clap)?;
    let command_matches = wrapped_matches.subcommand_matches("bdd").ok_or_else(|| {
        CliError::from(eyre::eyre!("Cargo's external subcommand parser lost `bdd`"))
    })?;
    let cli = Cli::from_arg_matches(command_matches).map_err(CliError::Clap)?;
    Ok((
        cli,
        command_matches.clone(),
        InvocationForm::CargoDispatched,
    ))
}

/// Parse the direct `cargo-bdd <command>` form.
fn parse_direct(arguments: Vec<OsString>) -> Result<(Cli, ArgMatches, InvocationForm), CliError> {
    let matches = Cli::command()
        .try_get_matches_from(arguments)
        .map_err(CliError::Clap)?;
    let cli = Cli::from_arg_matches(&matches).map_err(CliError::Clap)?;
    Ok((cli, matches, InvocationForm::Direct))
}

/// Recreate Clap's original parse failure for explicit `steps --json`.
///
/// Clap cannot enforce `requires` before `OrthoConfig` has merged files and
/// environment values. This parser is called only when the merged settings
/// still do not enable skipped-step reporting, and it returns the error rather
/// than exiting so the caller can inspect it.
pub(super) fn missing_required_skipped_error(invocation_form: InvocationForm) -> CliError {
    let command = Cli::command().mut_subcommand("steps", |steps| {
        steps.mut_arg("json", |argument| argument.requires("skipped"))
    });
    let (mut command, arguments) = match invocation_form {
        InvocationForm::Direct => (
            command,
            ["cargo-bdd", "steps", "--json"]
                .map(OsString::from)
                .to_vec(),
        ),
        InvocationForm::CargoDispatched => (
            external_subcommand("cargo-bdd", "bdd", command),
            ["cargo-bdd", "bdd", "steps", "--json"]
                .map(OsString::from)
                .to_vec(),
        ),
    };
    match command.try_get_matches_from_mut(arguments) {
        Err(error) => CliError::Clap(error),
        Ok(_matches) => CliError::Clap(command.error(
            clap::error::ErrorKind::MissingRequiredArgument,
            "the following required arguments were not provided:\n  --skipped",
        )),
    }
}
