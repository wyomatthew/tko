//! `tko` — a ticket management CLI backed by a git-service issue tracker.

mod backend;
mod cli;
mod commands;
mod config;
mod error;
mod ticket;

use clap::Parser;

use crate::backend::Backend;
use crate::backend::gitea::GiteaBackend;
use crate::cli::Cli;
use crate::config::{BackendKind, Config};

fn main() -> anyhow::Result<()> {
    let cli = Cli::parse();
    let config = Config::discover(&std::env::current_dir()?)?;
    let token = config.resolve_token()?;
    let backend = build_backend(&config, token)?;
    commands::dispatch(backend.as_ref(), cli.json, cli.command)
}

fn build_backend(config: &Config, token: String) -> anyhow::Result<Box<dyn Backend>> {
    match config.backend.kind {
        BackendKind::Gitea => Ok(Box::new(GiteaBackend::new(config, token)?)),
        BackendKind::Github => anyhow::bail!("the github backend is not yet implemented"),
        BackendKind::Gitlab => anyhow::bail!("the gitlab backend is not yet implemented"),
    }
}
