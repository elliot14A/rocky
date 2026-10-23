use std::path::{Path, PathBuf};

use clap::Args;
use tap::prelude::*;

use crate::error::Result;
use crate::{crypto, nix, ui};

#[derive(Args, Debug, Clone)]
pub struct RekeyArgs {
    /// Target environment (auto-detected if only one exists)
    #[arg(short, long)]
    pub env: Option<String>,

    /// Path to identity file for rekeying
    #[arg(short, long)]
    pub identity: Option<PathBuf>,
}

pub async fn run(args: RekeyArgs, target: Option<&Path>) -> Result<()> {
    nix::with_fleet(target, |fleet_root| async move {
        execute(args, &fleet_root).await
    })
    .await
}

async fn execute(args: RekeyArgs, fleet_root: &Path) -> Result<()> {
    let (env_name, env_dir) = nix::resolve_environment(fleet_root, args.env.as_deref())?;
    let secrets_dir = nix::resolve_secrets_dir(&env_dir);

    crypto::rekey(&secrets_dir, args.identity.as_deref())
        .await
        .tap_ok(|_| {
            ui::success(format!("Rekeyed secrets in environment '{env_name}'"));
        })?;

    Ok(())
}
