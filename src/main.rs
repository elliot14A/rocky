use std::process::ExitCode;

use clap::Parser;

use rocky::commands::{self, Command};
use rocky::process::log;

#[derive(Parser, Debug)]
#[command(name = "rocky", version, about = "NixOS fleet deployment tool")]
#[command(propagate_version = true)]
pub struct Cli {
    #[arg(short, long, global = true)]
    pub verbose: bool,

    #[command(subcommand)]
    pub command: Command,
}

#[tokio::main]
async fn main() -> ExitCode {
    let cli = Cli::parse();

    log::init(cli.verbose);

    if let Err(err) = commands::execute(cli.command).await {
        tracing::error!(%err, "command failed");
        return ExitCode::FAILURE;
    }

    ExitCode::SUCCESS
}
