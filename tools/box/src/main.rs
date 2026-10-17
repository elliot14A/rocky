use std::path::PathBuf;
use std::process::ExitCode;

use clap::{Parser, Subcommand};
use rocky::process::log;
use rocky::ui;
use tap::prelude::*;

pub mod error;
pub mod image;
pub mod meta;
pub mod start;
pub mod stop;

#[derive(Parser, Debug)]
#[command(
    name = "rocky-box",
    version,
    about = "Local test box runner for Rocky fleets"
)]
struct Cli {
    #[arg(short, long, global = true)]
    pub verbose: bool,

    #[command(subcommand)]
    command: Commands,
}

#[derive(Subcommand, Debug)]
enum Commands {
    /// Start a local virtual box
    Start {
        /// Name of the box (e.g. node-01)
        name: String,

        /// Forwarded SSH port on host (default: 2222)
        #[arg(short, long, default_value_t = 2222)]
        port: u16,

        /// Disk size in GB (default: 10)
        #[arg(short, long, default_value_t = 10)]
        disk_size: u64,

        /// Boot directly from installed disk (skip rescue ISO)
        #[arg(long)]
        installed: bool,

        /// Run QEMU in background (detached)
        #[arg(short, long)]
        detach: bool,

        /// Custom ISO path (defaults to Alpine virt ISO)
        #[arg(long)]
        iso: Option<PathBuf>,
    },

    /// Stop a running box
    Stop {
        /// Name of the box
        name: String,
    },
}

#[tokio::main]
async fn main() -> ExitCode {
    let cli = Cli::parse();
    log::init(cli.verbose);

    let res = match cli.command {
        Commands::Start {
            name,
            port,
            disk_size,
            installed,
            detach,
            iso,
        } => {
            let args = start::StartArgs {
                name,
                port,
                disk_size_gb: disk_size,
                installed,
                detach,
                iso,
            };
            start::start_box(&args).await
        }
        Commands::Stop { name } => stop::stop_box(&name).await,
    };

    res.tap_err(|err| ui::error(err.to_string()))
        .map(|_| ExitCode::SUCCESS)
        .unwrap_or(ExitCode::FAILURE)
}
