pub mod env;
pub mod host;

use clap::{Args, Subcommand};

use crate::error::Result;

#[derive(Args, Debug, Clone)]
pub struct NewArgs {
    #[command(subcommand)]
    pub command: NewCommand,
}

#[derive(Subcommand, Debug, Clone)]
pub enum NewCommand {
    /// Scaffold a new environment
    Env(env::EnvArgs),

    /// Scaffold a new host in an environment
    Host(host::HostArgs),
}

pub async fn run(args: NewArgs) -> Result<()> {
    match args.command {
        NewCommand::Env(args) => env::run(args, None).await,
        NewCommand::Host(args) => host::run(args, None).await,
    }
}
