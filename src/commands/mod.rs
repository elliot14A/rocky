pub mod init;

use clap::Subcommand;

use crate::error::Result;

#[derive(Subcommand, Debug, Clone)]
pub enum Command {
    /// Initialize a new fleet from the official GitHub templates catalog
    Init(init::InitArgs),
}

pub async fn execute(command: Command) -> Result<()> {
    match command {
        Command::Init(args) => init::run(args).await,
    }
}
