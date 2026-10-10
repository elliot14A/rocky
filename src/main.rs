use clap::Parser;
use rocky::Result;
use tracing_subscriber::EnvFilter;

#[derive(Debug, Parser)]
#[command(name = "rocky", version, about = "NixOS fleet deployment tool")]
pub struct Cli {
    #[arg(short, long)]
    pub verbose: bool,
}

#[tokio::main]
async fn main() -> Result<()> {
    tracing_subscriber::fmt()
        .with_env_filter(EnvFilter::from_default_env())
        .init();

    let _cli = Cli::parse();

    Ok(())
}
