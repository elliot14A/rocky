use std::path::Path;

use clap::Args;

use crate::error::Result;
use crate::nix::with_fleet;
use crate::process::mesh;
use crate::ui;

#[derive(Args, Debug, Clone)]
pub struct JoinArgs {
    #[arg(help = "Environment name to join (e.g. prod, staging)")]
    pub env: Option<String>,

    #[arg(short, long, help = "Tailscale authkey for authentication")]
    pub authkey: Option<String>,

    #[arg(long, help = "Override login server URL")]
    pub login_server: Option<String>,

    #[arg(long, help = "Force re-authentication when switching login servers")]
    pub force_reauth: bool,
}

pub async fn run(args: JoinArgs, target: Option<&Path>) -> Result<()> {
    with_fleet(target, |fleet_root| async move {
        execute(args, &fleet_root).await
    })
    .await
}

async fn execute(args: JoinArgs, fleet_root: &Path) -> Result<()> {
    let (env_name, _env_dir) = crate::nix::resolve_environment(fleet_root, args.env.as_deref())?;
    let domain = mesh::resolve_domain(fleet_root, &env_name).await?;

    let (is_already_joined, self_ip) =
        mesh::check_workstation_tailscale(&domain, &env_name).await?;
    if is_already_joined && !args.force_reauth {
        let ip = self_ip.as_deref().unwrap_or("-");
        ui::success(format!(
            "Workstation is already joined to fleet mesh '{env_name}' (IP: {ip})"
        ));
        ui::next_steps(&[
            "Check mesh status:  rocky mesh status".to_string(),
            format!("Connect to host:    rocky ssh {env_name}-<host>"),
            "Disconnect mesh:    rocky mesh leave".to_string(),
        ]);
        return Ok(());
    }

    let server = match args.login_server {
        Some(s) => s,
        None => mesh::resolve_login_server(fleet_root, &env_name).await?,
    };

    ui::info(format!("Resolving mesh authentication for '{env_name}'..."));
    let authkey = mesh::resolve_or_create_authkey(&server, args.authkey.as_deref()).await?;

    let force_reauth = args.force_reauth;
    ui::with_spinner(
        format!("Joining fleet mesh for '{env_name}' at {server}..."),
        format!("Joined fleet mesh for '{env_name}'"),
        || mesh::mesh_join(&server, &authkey, force_reauth),
    )
    .await?;

    ui::next_steps(&[
        "Check mesh status:  rocky mesh status".to_string(),
        format!("Connect to host:    rocky ssh {env_name}-<host>"),
        "Disconnect mesh:    rocky mesh leave".to_string(),
    ]);

    Ok(())
}
