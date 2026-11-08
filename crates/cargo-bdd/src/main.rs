//! Command line diagnostic tooling for rstest-bdd.

mod cli;
mod config;
mod observability;
mod output;
mod registry;

use cli::CliError;

/// Run the selected diagnostic, printing parse failures as Clap renders them.
fn main() -> eyre::Result<()> {
    observability::install();
    match cli::run() {
        Ok(()) => Ok(()),
        Err(CliError::Clap(error)) => error.exit(),
        Err(CliError::Report(report)) => Err(report),
    }
}
