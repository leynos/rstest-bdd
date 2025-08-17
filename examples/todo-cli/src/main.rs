//! Command-line interface for the `todo-cli` example.
//! Tasks live only in memory; each invocation starts with an empty to-do list.

use clap::builder::ValueParser;
use clap::{Parser, Subcommand};
use eyre::Result;
use todo_cli::TodoList;

/// Trim and reject blank-only task descriptions.
fn non_blank_string() -> ValueParser {
    ValueParser::from(|s: &str| {
        if s.trim().is_empty() {
            Err(String::from("task description must not be blank"))
        } else {
            Ok(String::from(s))
        }
    })
}

#[derive(Parser)]
#[command(author, version, about)]
struct Cli {
    #[command(subcommand)]
    command: Command,
}

#[derive(Subcommand)]
enum Command {
    /// Add a task to the list.
    Add {
        /// Task description (must not be empty).
        #[arg(value_parser = non_blank_string())]
        description: String,
    },
    /// Display all tasks.
    List,
}

fn main() -> Result<()> {
    let cli = Cli::parse();
    // Tasks persist only for this process; each run starts with an empty list.
    let mut list = TodoList::new();
    match cli.command {
        Command::Add { description } => list.add(description),
        Command::List => println!("{}", list.display()),
    }
    Ok(())
}
