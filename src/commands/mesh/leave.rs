use std::path::Path;

use clap::Args;

use crate::error::Result;
use crate::process::mesh;
use crate::ui;

#[derive(Args, Debug, Clone)]
pub struct LeaveArgs {
    #[arg(short, long, help = "Previous Tailscale profile to switch back to")]
    pub switch_to: Option<String>,
}

pub async fn run(args: LeaveArgs, _target: Option<&Path>) -> Result<()> {
    ui::with_spinner(
        "Disconnecting from fleet mesh...",
        "Disconnected from fleet mesh",
        || mesh::mesh_leave(args.switch_to.as_deref()),
    )
    .await?;

    Ok(())
}
