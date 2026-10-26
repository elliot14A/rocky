pub mod join;
pub mod leave;
pub mod status;

use std::path::Path;

use clap::{Args, Subcommand};

use crate::error::Result;

#[derive(Args, Debug, Clone)]
pub struct MeshArgs {
    #[command(subcommand)]
    pub command: MeshCommand,
}

#[derive(Subcommand, Debug, Clone)]
pub enum MeshCommand {
    /// Connect this machine to the fleet mesh network
    Join(join::JoinArgs),

    /// Check connection status and peers on the fleet mesh
    Status(status::StatusArgs),

    /// Disconnect from the fleet mesh and restore previous profile
    Leave(leave::LeaveArgs),
}

pub async fn run(args: MeshArgs, target: Option<&Path>) -> Result<()> {
    match args.command {
        MeshCommand::Join(args) => join::run(args, target).await,
        MeshCommand::Status(args) => status::run(args, target).await,
        MeshCommand::Leave(args) => leave::run(args, target).await,
    }
}
