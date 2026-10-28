use std::path::{Path, PathBuf};

use clap::Args;

use crate::error::Result;
use crate::nix::with_fleet;
use crate::process::mesh;
use crate::ui;

#[derive(Args, Debug, Clone, Default)]
pub struct JoinArgs {
    #[arg(value_name = "ENV", help = "Environment name to join (e.g. prod, staging)")]
    pub env: Option<String>,

    #[arg(
        short,
        long = "env",
        value_name = "ENV",
        conflicts_with = "env",
        help = "Target environment"
    )]
    pub env_flag: Option<String>,

    #[arg(short, long, help = "Tailscale authkey for authentication")]
    pub authkey: Option<String>,

    #[arg(short = 'i', long, help = "Path to SSH private key identity file for decrypting secrets")]
    pub identity: Option<PathBuf>,

    #[arg(long, help = "Override login server URL")]
    pub login_server: Option<String>,

    #[arg(long, help = "Force re-authentication when switching login servers")]
    pub force_reauth: bool,
}

impl JoinArgs {
    pub fn resolved_env(&self) -> Option<&str> {
        self.env.as_deref().or(self.env_flag.as_deref())
    }
}

pub async fn run(args: JoinArgs, target: Option<&Path>) -> Result<()> {
    with_fleet(target, |fleet_root| async move {
        execute(args, &fleet_root).await
    })
    .await
}

async fn execute(args: JoinArgs, fleet_root: &Path) -> Result<()> {
    let (env_name, _env_dir) = crate::nix::resolve_environment(fleet_root, args.resolved_env())?;
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
            format!("Connect to host:    rocky ssh {env_name} <host>"),
            "Disconnect mesh:    rocky mesh leave".to_string(),
        ]);
        return Ok(());
    }

    let server = match args.login_server {
        Some(s) => s,
        None => mesh::resolve_login_server(fleet_root, &env_name).await?,
    };

    ui::info(format!("Resolving mesh authentication for '{env_name}'..."));
    let authkey = mesh::resolve_authkey(
        fleet_root,
        &env_name,
        args.authkey.as_deref(),
        args.identity.as_deref(),
    )
    .await?;

    let force_reauth = args.force_reauth;
    ui::with_spinner(
        format!("Joining fleet mesh for '{env_name}' at {server}..."),
        format!("Joined fleet mesh for '{env_name}'"),
        || mesh::mesh_join(&server, &authkey, force_reauth),
    )
    .await?;

    ui::next_steps(&[
        "Check mesh status:  rocky mesh status".to_string(),
        format!("Connect to host:    rocky ssh {env_name} <host>"),
        "Disconnect mesh:    rocky mesh leave".to_string(),
    ]);

    Ok(())
}
