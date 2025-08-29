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
    match Cli::parse().command {
        Commands::Steps => handle_steps(),
        Commands::Unused => handle_unused(),
        Commands::Duplicates => handle_duplicates(),
    }
}

/// Handle the `steps` subcommand by listing all registered steps.
///
/// # Examples
///
/// ```no_run
/// handle_steps();
/// ```
fn handle_steps() {
    for step in iter::<Step> {
        print_step(step);
    }
}

/// Handle the `unused` subcommand by listing steps that were never executed.
///
/// # Examples
///
/// ```no_run
/// handle_unused();
/// ```
fn handle_unused() {
    for step in unused_steps() {
        print_step(step);
    }
}

/// Handle the `duplicates` subcommand by grouping identical step definitions.
///
/// # Examples
///
/// ```no_run
/// handle_duplicates();
/// ```
fn handle_duplicates() {
    for group in duplicate_steps() {
        for step in group {
            print_step(step);
        }
        println!("---");
    }
}

/// Print a step definition in diagnostic output.
///
/// # Examples
///
/// ```ignore
/// use rstest_bdd::{Step, StepKeyword, StepPattern};
///
/// static PATTERN: StepPattern = StepPattern::new("example");
/// let step = Step {
///     keyword: StepKeyword::Given,
///     pattern: &PATTERN,
///     run: todo!(),
///     fixtures: &[],
///     file: "src/example.rs",
///     line: 42,
/// };
/// print_step(&step);
/// ```
fn print_step(step: &Step) {
    println!(
        "{} '{}' ({}:{})",
        step.keyword.as_str(),
        step.pattern.as_str(),
        step.file,
        step.line
    );
}
