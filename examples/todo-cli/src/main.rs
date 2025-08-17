//! Command-line interface for the `todo-cli` example.

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
    Add { description: String },
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
