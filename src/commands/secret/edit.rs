use std::path::{Path, PathBuf};

use clap::Args;

use crate::error::Result;
use crate::{crypto, nix, ui};

#[derive(Args, Debug, Clone)]
pub struct EditArgs {
    /// Name of the secret (e.g. 'jwt' or 'jwt.age')
    pub name: String,

    /// Target environment (auto-detected if only one exists)
    #[arg(short, long)]
    pub env: Option<String>,

    /// Path to identity file for decryption
    #[arg(short, long)]
    pub identity: Option<PathBuf>,
}

pub async fn run(args: EditArgs, target: Option<&Path>) -> Result<()> {
    nix::with_fleet(target, |fleet_root| async move {
        execute(args, &fleet_root).await
    })
    .await
}

async fn execute(args: EditArgs, fleet_root: &Path) -> Result<()> {
    let (env_name, env_dir) = nix::resolve_environment(fleet_root, args.env.as_deref())?;

    let clean_name = args.name.strip_suffix(".age").unwrap_or(&args.name);
    let updated =
        crypto::secret::edit_secret(&env_dir, clean_name, args.identity.as_deref()).await?;
    if updated {
        ui::success(format!(
            "Updated secret '{clean_name}.age' in environment '{env_name}'"
        ));
    } else {
        ui::info(format!(
            "Secret '{clean_name}.age' unchanged in environment '{env_name}'"
        ));
    }

    Ok(())
}
