//! Command-line interface for the `todo-cli` example.
//! Tasks live only in memory; each invocation starts with an empty to-do list.

use clap::{Parser, Subcommand};
use eyre::Result;
use todo_cli::TodoList;

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
        #[arg(value_parser = clap::builder::NonEmptyStringValueParser::new())]
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
        Command::Add { description } => {
            if description.trim().is_empty() {
                eyre::bail!("task description must not be blank");
            }
            list.add(description)
        }
        Command::List => println!("{}", list.display()),
    }
    Ok(())
}
