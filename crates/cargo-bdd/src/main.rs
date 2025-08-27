//! Command-line interface for `rstest-bdd` diagnostics.

use clap::{Parser, Subcommand};
use eyre::Result;
use std::path::PathBuf;

use cargo_bdd::{duplicates, steps, unused};

/// Inspect `rstest-bdd` step definitions in the current crate.
#[derive(Parser)]
#[command(version, about)]
struct Cli {
    #[command(subcommand)]
    command: Commands,
}

#[derive(Subcommand)]
enum Commands {
    /// List all registered step definitions.
    #[command(name = "list-steps")]
    Steps,
    /// Report duplicate step definitions.
    #[command(name = "list-duplicates")]
    Duplicates,
    /// Show step definitions unused by the supplied feature files.
    #[command(name = "list-unused")]
    Unused { paths: Vec<PathBuf> },
}

fn main() -> Result<()> {
    let cli = Cli::parse();
    match cli.command {
        Commands::Steps => {
            for step in steps() {
                println!(
                    "{} {} ({}:{})",
                    step.keyword.as_str(),
                    step.pattern.as_str(),
                    step.file,
                    step.line
                );
            }
        }
        Commands::Duplicates => {
            for group in duplicates() {
                let first = group[0];
                println!("{} {}", first.keyword.as_str(), first.pattern.as_str());
                for step in group {
                    println!("  {}:{}", step.file, step.line);
                }
            }
        }
        Commands::Unused { paths } => {
            for step in unused(&paths)? {
                println!(
                    "{} {} ({}:{})",
                    step.keyword.as_str(),
                    step.pattern.as_str(),
                    step.file,
                    step.line
                );
            }
        }
    }
    Ok(())
}

// Sample steps for tests; gated behind feature.
#[cfg(feature = "test-steps")]
mod test_steps;
