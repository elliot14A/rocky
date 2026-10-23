use std::process::ExitCode;

use clap::Parser;
use tap::prelude::*;

use rocky::commands::{self, Cli};
use rocky::process::log;
use rocky::ui;

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
