//! Command-line diagnostic tooling for rstest-bdd.

use clap::{Parser, Subcommand};
use rstest_bdd::{Step, duplicate_steps, iter, unused_steps};

/// Cargo subcommand providing diagnostics for rstest-bdd.
#[derive(Parser)]
#[command(author, version, about)]
struct Cli {
    #[command(subcommand)]
    command: Commands,
}

/// Supported diagnostic commands.
#[derive(Subcommand)]
enum Commands {
    /// List all registered steps.
    Steps,
    /// List registered steps that were never executed.
    Unused,
    /// List step definitions that share the same keyword and pattern.
    Duplicates,
}

fn main() {
    let cli = Cli::parse();
    match cli.command {
        Commands::Steps => {
            for step in iter::<Step> {
                print_step(step);
            }
        }
        Commands::Unused => {
                for step in unused_steps() {
                    print_step(step);
                }
        }
        Commands::Duplicates => {
            for group in duplicate_steps() {
                for step in group {
                    print_step(step);
                }
                println!("---");
            }
        }
    }
}

fn print_step(step: &Step) {
    println!(
        "{} '{}' ({}:{})",
        step.keyword.as_str(),
        step.pattern.as_str(),
        step.file,
        step.line
    );
}
