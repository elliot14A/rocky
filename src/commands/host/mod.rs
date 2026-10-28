pub mod edit;
pub mod list;
pub mod new;

use std::path::Path;

use clap::{Args, Subcommand};

use crate::error::Result;

#[derive(Args, Debug, Clone)]
pub struct HostArgs {
    #[command(subcommand)]
    pub command: HostCommand,
}

#[derive(Subcommand, Debug, Clone)]
pub enum HostCommand {
    /// Scaffold a new host in an environment
    New(new::NewArgs),

    /// List all fleet hosts
    List(list::ListArgs),

    /// Edit host configuration in an environment
    Edit(edit::EditArgs),
}

pub async fn run(args: HostArgs, target: Option<&Path>) -> Result<()> {
    match args.command {
        HostCommand::New(args) => new::run(args, target).await,
        HostCommand::List(args) => list::run(args, target).await,
        HostCommand::Edit(args) => edit::run(args, target).await,
    }
}
