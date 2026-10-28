use std::path::{Path, PathBuf};

use clap::Args;

use crate::error::Result;
use crate::nix::with_fleet;
use crate::process::journal::{self, JournalOptions};

#[derive(Args, Debug, Clone)]
pub struct LogsArgs {
    #[arg(help = "Target environment name (e.g. prod, staging)")]
    pub env: String,

    #[arg(help = "Service name to stream logs for")]
    pub service: Option<String>,

    #[arg(long, help = "Target specific host within the environment")]
    pub host: Option<String>,

    #[arg(short = 'i', long, help = "Path to SSH private key identity file")]
    pub identity: Option<PathBuf>,

    #[arg(short = 'f', long, help = "Follow journal log output")]
    pub follow: bool,

    #[arg(short = 'n', long, help = "Number of journal lines to show")]
    pub tail: Option<usize>,

    #[arg(
        long,
        help = "Show entries not older than the specified time (e.g. '1h ago', '2026-10-26')"
    )]
    pub since: Option<String>,

    #[arg(long, help = "Show entries not newer than the specified time")]
    pub until: Option<String>,

    #[arg(short = 't', long, help = "Show ISO timestamps for entries")]
    pub timestamps: bool,
}

pub async fn run(args: LogsArgs, target_dir: Option<&Path>) -> Result<()> {
    let opts = JournalOptions {
        follow: args.follow,
        tail: args.tail,
        since: args.since,
        until: args.until,
        timestamps: args.timestamps,
    };

    with_fleet(target_dir, |fleet_root| async move {
        journal::execute_logs(
            &fleet_root,
            &args.env,
            args.service.as_deref(),
            args.host.as_deref(),
            args.identity.as_deref(),
            &opts,
        )
        .await
    })
    .await
}
