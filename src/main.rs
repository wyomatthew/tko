//! `tko` — a ticket management CLI backed by a git-service issue tracker.

mod cli;
mod config;
mod error;

use clap::Parser;

use crate::cli::{Cli, Command, DepCommand};

fn main() -> anyhow::Result<()> {
    let cli = Cli::parse();
    run(cli)
}

/// Dispatch a parsed [`Cli`] to its command handler.
///
/// Command bodies are implemented in their respective tickets; for now each
/// dispatch arm reports that the command is not yet implemented so the CLI
/// skeleton is exercisable end to end.
fn run(cli: Cli) -> anyhow::Result<()> {
    let name = match &cli.command {
        Command::Create { .. } => "create",
        Command::Show { .. } => "show",
        Command::List { .. } => "list",
        Command::Ready { .. } => "ready",
        Command::Blocked { .. } => "blocked",
        Command::Closed { .. } => "closed",
        Command::Start { .. } => "start",
        Command::Close { .. } => "close",
        Command::Reopen { .. } => "reopen",
        Command::Status { .. } => "status",
        Command::Edit { .. } => "edit",
        Command::AddNote { .. } => "add-note",
        Command::Dep { command } => match command {
            DepCommand::Add { .. } => "dep add",
            DepCommand::Remove { .. } => "dep remove",
            DepCommand::Tree { .. } => "dep tree",
            DepCommand::Check => "dep check",
        },
        Command::Undep { .. } => "undep",
        Command::Link { .. } => "link",
        Command::Unlink { .. } => "unlink",
    };

    anyhow::bail!("`{name}` is not yet implemented");
}
