use std::process::ExitCode;

use clap::Parser;
use tap::prelude::*;

use rocky::commands::{self, Command};
use rocky::process::log;
use rocky::ui;

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

    commands::execute(cli.command)
        .await
        .tap_err(|err| {
            ui::error(err.to_string());
            tracing::error!(%err, "command failed");
        })
        .map(|_| ExitCode::SUCCESS)
        .unwrap_or(ExitCode::FAILURE)
}
