use std::path::Path;

use clap::Args;

use crate::error::Result;
use crate::nix::with_fleet;
use crate::process::ssh;

#[derive(Args, Debug, Clone)]
pub struct SshArgs {
    #[arg(help = "Target host in <env>-<host> format (e.g. prod-db)")]
    pub target: String,

    #[arg(
        trailing_var_arg = true,
        allow_hyphen_values = true,
        help = "Additional arguments or commands passed directly to ssh"
    )]
    pub args: Vec<String>,
}

pub async fn run(args: SshArgs, target_dir: Option<&Path>) -> Result<()> {
    with_fleet(target_dir, |fleet_root| async move {
        ssh::execute_ssh(&fleet_root, &args.target, &args.args).await
    })
    .await
}
