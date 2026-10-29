use std::path::{Path, PathBuf};

use clap::Args;

use crate::error::Result;
use crate::nix;

#[derive(Args, Debug, Clone)]
pub struct TuiArgs {
    /// Path to SSH private key identity file
    #[arg(short = 'i', long, help = "Path to SSH private key identity file")]
    pub identity: Option<PathBuf>,
}

pub async fn run(args: TuiArgs, target_dir: Option<&Path>) -> Result<()> {
    nix::with_fleet(target_dir, |fleet_root| async move {
        execute(args, &fleet_root).await
    })
    .await
}

async fn execute(args: TuiArgs, fleet_root: &Path) -> Result<()> {
    crate::tui::run(fleet_root, args.identity.as_deref()).await
}
