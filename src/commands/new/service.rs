use std::path::Path;

use clap::{Args, ValueEnum};

use crate::error::Result;
use crate::templates::service::ServiceCreationResult;
use crate::{nix, templates, ui};

#[derive(Debug, Clone, Copy, PartialEq, Eq, ValueEnum)]
#[value(rename_all = "snake_case")]
pub enum ServiceKind {
    Podman,
    Systemd,
}

impl std::fmt::Display for ServiceKind {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Podman => write!(f, "podman"),
            Self::Systemd => write!(f, "systemd"),
        }
    }
}

#[derive(Args, Debug, Clone)]
pub struct ServiceArgs {
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

pub async fn run(args: ServiceArgs, target: Option<&Path>) -> Result<()> {
    nix::with_fleet(target, |fleet_root| async move {
        execute(args, &fleet_root).await
    })
    .await
}

async fn execute(args: ServiceArgs, fleet_root: &Path) -> Result<()> {
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
                "Fleet has multiple hosts [{}]. Assign the service with 'rocky new service {} --host <name>' or in environments/{}/fleet.nix",
                hosts.join(", "),
                args.name,
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
