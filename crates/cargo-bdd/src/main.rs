//! Command line diagnostic tooling for rstest-bdd.

mod cli;
mod output;
mod registry;

fn main() -> eyre::Result<()> {
    cli::run()
}
