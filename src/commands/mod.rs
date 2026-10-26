pub mod check;
pub mod completion;
pub mod init;
pub mod mesh;
pub mod new;
pub mod secret;
pub mod up;

use clap::{Parser, Subcommand};

use crate::error::Result;

#[derive(Parser, Debug, Clone)]
#[command(name = "rocky", version, about = "NixOS fleet deployment tool")]
#[command(propagate_version = true)]
pub struct Cli {
    #[arg(short, long, global = true)]
    pub verbose: bool,

    #[command(subcommand)]
    pub command: Command,
}

#[derive(Subcommand, Debug, Clone)]
pub enum Command {
    /// Check fleet configurations and flake outputs
    Check(check::CheckArgs),

    /// Generate shell completion scripts
    Completion(completion::CompletionArgs),

    /// Initialize a new fleet from the official GitHub templates catalog
    Init(init::InitArgs),

    /// Connect, inspect, or leave the fleet mesh network
    Mesh(mesh::MeshArgs),

    /// Scaffold a new entity (host, env, service)
    New(new::NewArgs),

    /// Manage encrypted fleet secrets
    Secret(secret::SecretArgs),

    /// Deploy and install fleet hosts
    Up(up::UpArgs),
}

pub async fn execute(command: Command) -> Result<()> {
    match command {
        Command::Check(args) => check::run(args, None).await,
        Command::Completion(args) => completion::run(args),
        Command::Init(args) => init::run(args).await,
        Command::Mesh(args) => mesh::run(args, None).await,
        Command::New(args) => new::run(args).await,
        Command::Secret(args) => secret::run(args, None).await,
        Command::Up(args) => up::run(args, None).await,
    }
}
