pub mod init;
pub mod new;
pub mod secret;
pub mod up;

use clap::Subcommand;

use crate::error::Result;

#[derive(Subcommand, Debug, Clone)]
pub enum Command {
    /// Initialize a new fleet from the official GitHub templates catalog
    Init(init::InitArgs),

    /// Scaffold a new entity (host, env, service)
    New(new::NewArgs),

    /// Manage encrypted fleet secrets
    Secret(secret::SecretArgs),

    /// Deploy and install fleet hosts
    Up(up::UpArgs),
}

pub async fn execute(command: Command) -> Result<()> {
    match command {
        Command::Init(args) => init::run(args).await,
        Command::New(args) => new::run(args).await,
        Command::Secret(args) => secret::run(args, None).await,
        Command::Up(args) => up::run(args, None).await,
    }
}
