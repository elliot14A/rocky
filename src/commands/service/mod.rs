pub mod edit;
pub mod list;
pub mod new;

pub use crate::templates::service::ServiceKind;

use std::path::Path;

use clap::{Args, Subcommand};

use crate::error::Result;

#[derive(Args, Debug, Clone)]
pub struct ServiceArgs {
    #[command(subcommand)]
    pub command: ServiceCommand,
}

#[derive(Subcommand, Debug, Clone)]
pub enum ServiceCommand {
    /// Scaffold a new service module
    New(new::NewArgs),

    /// List all fleet services
    List(list::ListArgs),

    /// Edit a service module
    Edit(edit::EditArgs),
}

pub async fn run(args: ServiceArgs, target: Option<&Path>) -> Result<()> {
    match args.command {
        ServiceCommand::New(args) => new::run(args, target).await,
        ServiceCommand::List(args) => list::run(args, target).await,
        ServiceCommand::Edit(args) => edit::run(args, target).await,
    }
}
