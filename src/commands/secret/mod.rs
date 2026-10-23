pub mod edit;
pub mod new;
pub mod rekey;
pub mod show;

use std::path::Path;

use clap::{Args, Subcommand};

use crate::error::Result;

#[derive(Args, Debug, Clone)]
pub struct SecretArgs {
    #[command(subcommand)]
    pub command: SecretCommand,
}

#[derive(Subcommand, Debug, Clone)]
pub enum SecretCommand {
    /// Create a new encrypted .age secret and scope recipient keys
    New(new::NewArgs),

    /// Decrypt and display a secret or extract a single key
    Show(show::ShowArgs),

    /// Decrypt a secret into an interactive editor and re-encrypt on save
    Edit(edit::EditArgs),

    /// Rekey all secrets in an environment
    Rekey(rekey::RekeyArgs),
}

pub async fn run(args: SecretArgs, target: Option<&Path>) -> Result<()> {
    match args.command {
        SecretCommand::New(args) => new::run(args, target).await,
        SecretCommand::Show(args) => show::run(args, target).await,
        SecretCommand::Edit(args) => edit::run(args, target).await,
        SecretCommand::Rekey(args) => rekey::run(args, target).await,
    }
}
