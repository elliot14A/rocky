use std::path::Path;

use clap::Args;

use crate::error::Result;
use crate::nix::with_fleet;
use crate::process::mesh;
use crate::ui;

#[derive(Args, Debug, Clone, Default)]
pub struct StatusArgs {
    #[arg(value_name = "ENV", help = "Environment name to check (e.g. prod, staging)")]
    pub env: Option<String>,

    #[arg(
        short,
        long = "env",
        value_name = "ENV",
        conflicts_with = "env",
        help = "Target environment"
    )]
    pub env_flag: Option<String>,
}

impl StatusArgs {
    pub fn resolved_env(&self) -> Option<&str> {
        self.env.as_deref().or(self.env_flag.as_deref())
    }
}

pub async fn run(args: StatusArgs, target: Option<&Path>) -> Result<()> {
    with_fleet(target, |fleet_root| async move {
        execute(args, &fleet_root).await
    })
    .await
}

async fn execute(args: StatusArgs, fleet_root: &Path) -> Result<()> {
    let (env_name, _env_dir) = crate::nix::resolve_environment(fleet_root, args.resolved_env())?;
    let fleet_status = mesh::get_fleet_mesh_status(fleet_root, &env_name).await?;

    ui::info(format!("Environment:   {}", fleet_status.environment));
    ui::info(format!("Login Server:  {}", fleet_status.login_server));

    if fleet_status.workstation_connected {
        let ip = fleet_status.workstation_ip.as_deref().unwrap_or("-");
        ui::success(format!("Workstation:   Connected (IP: {ip})"));
    } else {
        ui::warn(format!(
            "Workstation:   Disconnected (Run 'rocky mesh join {env_name}' to connect)"
        ));
    }

    eprintln!();
    println!(
        "{:<16} {:<10} {:<18} {:<16}",
        "HOST", "STATUS", "MESH IP", "MAGICDNS"
    );
    println!(
        "{:-<16} {:-<10} {:-<18} {:-<16}",
        "", "", "", ""
    );

    for host in &fleet_status.hosts {
        println!(
            "{:<16} {:<10} {:<18} {:<16}",
            host.name, host.status, host.mesh_ip, host.dns_name
        );
    }
    eprintln!();

    Ok(())
}
