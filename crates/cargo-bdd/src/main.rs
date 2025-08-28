//! Command-line interface for `rstest-bdd` diagnostics.

use clap::{CommandFactory, Parser, Subcommand};
use eyre::Result;
use std::path::PathBuf;

use cargo_bdd::{duplicates, steps, unused};

/// Inspect `rstest-bdd` step definitions in the current crate.
#[derive(Parser)]
#[command(version, about)]
struct Cli {
    #[command(subcommand)]
    command: Option<Commands>,
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

/// Execute the `list-steps` command.
///
/// ```
/// # fn try_it() -> eyre::Result<()> {
/// execute_steps_command()?;
/// # Ok(())
/// # }
/// ```
fn execute_steps_command() -> Result<()> {
    for step in steps() {
        println!(
            "{} {} ({}:{})",
            step.keyword.as_str(),
            step.pattern.as_str(),
            step.file,
            step.line
        );
    }
    Ok(())
}

/// Execute the `list-duplicates` command.
///
/// ```
/// # fn try_it() -> eyre::Result<()> {
/// execute_duplicates_command()?;
/// # Ok(())
/// # }
/// ```
fn execute_duplicates_command() -> Result<()> {
    for group in duplicates() {
        let first = group[0];
        println!("{} {}", first.keyword.as_str(), first.pattern.as_str());
        for step in group {
            println!("  {}:{}", step.file, step.line);
        }
    }
    Ok(())
}

/// Execute the `list-unused` command for the given feature paths.
///
/// ```
/// # use std::path::PathBuf;
/// # fn try_it() -> eyre::Result<()> {
/// let paths: Vec<PathBuf> = Vec::new();
/// execute_unused_command(&paths)?;
/// # Ok(())
/// # }
/// ```
fn execute_unused_command(paths: &[PathBuf]) -> Result<()> {
    for step in unused(paths)? {
        println!(
            "{} {} ({}:{})",
            step.keyword.as_str(),
            step.pattern.as_str(),
            step.file,
            step.line
        );
    }
    Ok(())
}

/// Display help information when no subcommand is supplied.
///
/// ```
/// # fn try_it() -> eyre::Result<()> {
/// execute_help_command()?;
/// # Ok(())
/// # }
/// ```
fn execute_help_command() -> Result<()> {
    Cli::command().print_help()?;
    println!();
    Ok(())
}

fn main() -> Result<()> {
    let cli = Cli::parse();
    match cli.command {
        Some(Commands::Steps) => execute_steps_command(),
        Some(Commands::Duplicates) => execute_duplicates_command(),
        Some(Commands::Unused { paths }) => execute_unused_command(&paths),
        None => execute_help_command(),
    }
}

// Sample steps for tests; gated behind feature.
#[cfg(feature = "test-steps")]
mod test_steps;
