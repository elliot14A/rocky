use std::path::Path;

use clap::Args;

use crate::error::Result;
use crate::templates::service::{ServiceCreationResult, ServiceKind};
use crate::{nix, templates, ui};

#[derive(Args, Debug, Clone)]
pub struct NewArgs {
    /// Name of the service
    pub name: String,

    /// Assign service to this host in fleet.nix
    #[arg(long)]
    pub host: Option<String>,

    /// Environment to target (defaults to auto-detected environment)
    #[arg(short, long)]
    pub env: Option<String>,

    /// Kind of service to scaffold (systemd or podman)
    #[arg(short, long, value_enum, default_value = "systemd")]
    pub kind: ServiceKind,

    /// Number of service replicas (default: 1)
    #[arg(short, long, default_value = "1")]
    pub replicas: u32,

    /// Port or base port for the service
    #[arg(short, long)]
    pub port: Option<u16>,
}

pub type ServiceArgs = NewArgs;

pub async fn run(args: NewArgs, target: Option<&Path>) -> Result<()> {
    nix::with_fleet(target, |fleet_root| async move {
        execute(args, &fleet_root).await
    })
    .await
}

async fn execute(args: NewArgs, fleet_root: &Path) -> Result<()> {
    let result = templates::service::create(
        &args.name,
        args.kind,
        args.replicas,
        args.port,
        args.env.as_deref(),
        args.host.as_deref(),
        fleet_root,
    )
    .await?;

    match result {
        ServiceCreationResult::AssignedToHost { host, environment } => {
            ui::success(format!(
                "Created {} service '{}' and assigned to host '{}' in environment '{}'",
                args.kind, args.name, host, environment
            ));
        }
        ServiceCreationResult::MultipleHostsNotice { environment, hosts } => {
            ui::success(format!("Created {} service '{}'", args.kind, args.name));
            ui::info(format!(
                "Fleet has multiple hosts [{}]. Assign the service in environments/{}/fleet.nix or with 'rocky host edit <host>'",
                hosts.join(", "),
                environment
            ));
        }
        ServiceCreationResult::ScaffoldedOnly => {
            ui::success(format!("Created {} service '{}'", args.kind, args.name));
            ui::next_steps(&[
                format!("Configure the service in services/{}.nix", args.name),
                "Assign the service to a host in environments/<env>/fleet.nix".to_string(),
            ]);
        }
    }

    Ok(())
}
