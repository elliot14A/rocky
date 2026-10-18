pub mod init;
pub mod new;
pub mod up;

use clap::Subcommand;

use crate::error::Result;

#[derive(Subcommand, Debug, Clone)]
pub enum Command {
    /// Initialize a new fleet from the official GitHub templates catalog
    Init(init::InitArgs),

    /// Scaffold a new entity (host, env, service)
    New(new::NewArgs),
}

pub async fn execute(command: Command) -> Result<()> {
    match command {
        Command::Init(args) => init::run(args).await,
        Command::New(args) => new::run(args).await,
    }
}
