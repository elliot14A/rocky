pub mod check;
pub mod completion;
pub mod env;
pub mod host;
pub mod init;
pub mod logs;
pub mod mesh;
pub mod secret;
pub mod service;
pub mod ssh;
pub mod tui;
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

    /// Manage fleet environments
    Env(env::EnvArgs),

    /// Manage fleet hosts
    Host(host::HostArgs),

    /// Initialize a new fleet from the official GitHub templates catalog
    Init(init::InitArgs),

    /// Stream system and container journal logs over the mesh network
    Logs(logs::LogsArgs),

    /// Connect, inspect, or leave the fleet mesh network
    Mesh(mesh::MeshArgs),

    /// Manage encrypted fleet secrets
    Secret(secret::SecretArgs),

    /// Manage fleet services
    Service(service::ServiceArgs),

    /// Connect to a fleet host over the mesh network
    Ssh(ssh::SshArgs),

    /// Launch interactive fleet terminal interface
    Tui(tui::TuiArgs),

    /// Deploy and install fleet hosts
    Up(up::UpArgs),
}

pub async fn execute(command: Command) -> Result<()> {
    match command {
        Command::Check(args) => check::run(args, None).await,
        Command::Completion(args) => completion::run(args),
        Command::Env(args) => env::run(args, None).await,
        Command::Host(args) => host::run(args, None).await,
        Command::Init(args) => init::run(args).await,
        Command::Logs(args) => logs::run(args, None).await,
        Command::Mesh(args) => mesh::run(args, None).await,
        Command::Secret(args) => secret::run(args, None).await,
        Command::Service(args) => service::run(args, None).await,
        Command::Ssh(args) => ssh::run(args, None).await,
        Command::Tui(args) => tui::run(args, None).await,
        Command::Up(args) => up::run(args, None).await,
    }
}
