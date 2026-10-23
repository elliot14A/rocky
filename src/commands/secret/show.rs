use std::path::{Path, PathBuf};

use clap::Args;

use crate::error::Result;
use crate::{crypto, nix};

#[derive(Args, Debug, Clone)]
pub struct ShowArgs {
    /// Name of the secret (e.g. 'jwt' or 'jwt.age')
    pub name: String,

    /// Target environment (auto-detected if only one exists)
    #[arg(short, long)]
    pub env: Option<String>,

    /// Extract a specific key from key-value secret
    #[arg(short, long)]
    pub key: Option<String>,

    /// Path to identity file for decryption
    #[arg(short, long)]
    pub identity: Option<PathBuf>,
}

pub async fn run(args: ShowArgs, target: Option<&Path>) -> Result<()> {
    nix::with_fleet(target, |fleet_root| async move {
        execute(args, &fleet_root).await
    })
    .await
}

async fn execute(args: ShowArgs, fleet_root: &Path) -> Result<()> {
    let (_env_name, env_dir) = nix::resolve_environment(fleet_root, args.env.as_deref())?;

    let clean_name = args.name.strip_suffix(".age").unwrap_or(&args.name);
    let content = crypto::secret::show_secret(
        &env_dir,
        clean_name,
        args.identity.as_deref(),
        args.key.as_deref(),
    )
    .await?;

    if content.ends_with('\n') {
        print!("{content}");
    } else {
        println!("{content}");
    }

    Ok(())
}
