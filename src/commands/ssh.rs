use std::path::{Path, PathBuf};

use clap::Args;

use crate::error::Result;
use crate::nix::with_fleet;
use crate::process::ssh;

#[derive(Args, Debug, Clone)]
pub struct SshArgs {
    #[arg(help = "Target environment name (e.g. prod, staging)")]
    pub env: String,

    #[arg(help = "Target host or service name within the environment (e.g. db, edge, caddy)")]
    pub target: String,

    #[arg(short, long, help = "Path to SSH private key identity file")]
    pub identity: Option<PathBuf>,

    #[arg(
        trailing_var_arg = true,
        allow_hyphen_values = true,
        help = "Additional arguments or commands passed directly to ssh"
    )]
    pub args: Vec<String>,
}

pub async fn run(args: SshArgs, target_dir: Option<&Path>) -> Result<()> {
    with_fleet(target_dir, |fleet_root| async move {
        ssh::execute_ssh(
            &fleet_root,
            &args.env,
            &args.target,
            args.identity.as_deref(),
            &args.args,
        )
        .await
    })
    .await
}
