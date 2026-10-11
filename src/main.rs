use std::process::ExitCode;

use clap::Parser;
use rocky::process::log;

#[derive(Parser, Debug)]
#[command(name = "rocky", version, about = "NixOS fleet deployment tool")]
pub struct Cli {
    #[arg(short, long, global = true)]
    pub verbose: bool,
}

#[tokio::main]
async fn main() -> ExitCode {
    let cli = Cli::parse();
    log::init(cli.verbose);
    ExitCode::SUCCESS
}
