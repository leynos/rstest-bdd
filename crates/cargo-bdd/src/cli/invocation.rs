//! Direct and Cargo-dispatched argument parsing for the `cargo-bdd` binary.
//!
//! This module owns the parser wrapper for the application binary and preserves
//! the prior Clap error for a `steps --json` invocation that has no effective
//! `skipped` setting. It is private to `cargo-bdd` and is called by `cli::run`.

use std::ffi::OsString;

use clap::{ArgMatches, CommandFactory, FromArgMatches};
use eyre::Result;
use ortho_config::cargo::external_subcommand;

use super::Cli;

/// Entry shape used to select the parser and its compatibility output.
#[derive(Clone, Copy)]
pub(super) enum InvocationForm {
    /// The binary was called directly as `cargo-bdd <subcommand>`.
    Direct,
    /// Cargo injected `bdd` before the command name.
    CargoDispatched,
}

/// Parse either the direct binary form or Cargo's injected `bdd` form.
pub(super) fn parse_command_line<I>(arguments: I) -> Result<(Cli, ArgMatches, InvocationForm)>
where
    I: IntoIterator<Item = OsString>,
{
    let arguments = arguments.into_iter().collect::<Vec<_>>();
    let is_cargo_dispatch = arguments.get(1).is_some_and(|argument| argument == "bdd");
    if is_cargo_dispatch {
        let wrapped_matches =
            external_subcommand("cargo-bdd", "bdd", Cli::command()).get_matches_from(arguments);
        let command_matches = wrapped_matches
            .subcommand_matches("bdd")
            .ok_or_else(|| eyre::eyre!("Cargo's external subcommand parser lost `bdd`"))?;
        let cli = Cli::from_arg_matches(command_matches).unwrap_or_else(|error| error.exit());
        Ok((
            cli,
            command_matches.clone(),
            InvocationForm::CargoDispatched,
        ))
    } else {
        let matches = Cli::command().get_matches_from(arguments);
        let cli = Cli::from_arg_matches(&matches).unwrap_or_else(|error| error.exit());
        Ok((cli, matches, InvocationForm::Direct))
    }
}

/// Recreate Clap's original parse failure for explicit `steps --json`.
///
/// Clap cannot enforce `requires` before `OrthoConfig` has merged files and
/// environment values. This parser is called only when the merged settings
/// still do not enable skipped-step reporting.
pub(super) fn missing_required_skipped_error(invocation_form: InvocationForm) -> ! {
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
    if let Err(error) = command.try_get_matches_from_mut(arguments) {
        error.exit();
    }
    command
        .error(
            clap::error::ErrorKind::MissingRequiredArgument,
            "the following required arguments were not provided:\n  --skipped",
        )
        .exit()
}
