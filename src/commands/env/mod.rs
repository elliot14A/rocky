pub mod edit;
pub mod list;
pub mod new;

use std::path::Path;

use clap::{Args, Subcommand};

use crate::error::Result;

#[derive(Args, Debug, Clone)]
pub struct EnvArgs {
    #[command(subcommand)]
    pub command: EnvCommand,
}

#[derive(Subcommand, Debug, Clone)]
pub enum EnvCommand {
    /// Scaffold a new environment
    New(new::NewArgs),

    /// List all fleet environments
    List(list::ListArgs),

    /// Edit fleet configuration for an environment
    Edit(edit::EditArgs),
}

pub async fn run(args: EnvArgs, target: Option<&Path>) -> Result<()> {
    match args.command {
        EnvCommand::New(args) => new::run(args, target).await,
        EnvCommand::List(args) => list::run(args, target).await,
        EnvCommand::Edit(args) => edit::run(args, target).await,
    }
}
