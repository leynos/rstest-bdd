//! Command line diagnostic tooling for rstest-bdd.

mod cli;
mod config;
mod output;
mod registry;

fn main() -> eyre::Result<()> { cli::run() }
