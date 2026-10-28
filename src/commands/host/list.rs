use std::path::Path;

use clap::Args;

use crate::error::Result;
use crate::{fs, nix, ui};

#[derive(Args, Debug, Clone, Default)]
pub struct ListArgs {
    /// Target environment to inspect
    #[arg(value_name = "ENV", help = "Target environment to inspect")]
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

impl ListArgs {
    pub fn resolved_env(&self) -> Option<&str> {
        self.env.as_deref().or(self.env_flag.as_deref())
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct HostInfo {
    pub name: String,
    pub environment: String,
    pub services: Vec<String>,
}

pub async fn run(args: ListArgs, target: Option<&Path>) -> Result<()> {
    nix::with_fleet(target, |fleet_root| async move {
        execute(args, &fleet_root).await
    })
    .await
}

pub fn collect_hosts(
    fleet_root: &Path,
    env_filter: Option<&str>,
) -> Result<Vec<HostInfo>> {
    let mut entries = Vec::new();

    if let Some(env_name) = env_filter {
        let (resolved_name, env_dir) = nix::resolve_environment(fleet_root, Some(env_name))?;
        collect_hosts_from_env(&resolved_name, &env_dir, &mut entries)?;
    } else {
        let env_dirs = nix::list_environments(fleet_root)?;
        for (name, env_dir) in env_dirs {
            let _ = collect_hosts_from_env(&name, &env_dir, &mut entries);
        }
    }

    entries.sort_by(|a, b| {
        a.environment
            .cmp(&b.environment)
            .then_with(|| a.name.cmp(&b.name))
    });
    Ok(entries)
}

async fn execute(args: ListArgs, fleet_root: &Path) -> Result<()> {
    let entries = collect_hosts(fleet_root, args.resolved_env())?;

    if entries.is_empty() {
        if let Some(env_name) = args.resolved_env() {
            ui::warn(format!("No hosts found in environment '{env_name}'"));
        } else {
            ui::warn("No hosts found in fleet");
        }
        return Ok(());
    }

    println!("{:<16} {:<16} {:<32}", "HOST", "ENVIRONMENT", "SERVICES");
    println!("{:-<16} {:-<16} {:-<32}", "", "", "");

    for entry in entries {
        let services_str = if entry.services.is_empty() {
            "-".to_string()
        } else {
            entry.services.join(", ")
        };
        println!("{:<16} {:<16} {:<32}", entry.name, entry.environment, services_str);
    }

    Ok(())
}

fn collect_hosts_from_env(
    env_name: &str,
    env_dir: &Path,
    entries: &mut Vec<HostInfo>,
) -> Result<()> {
    let fleet_nix = env_dir.join("fleet.nix");
    if !fleet_nix.is_file() {
        return Ok(());
    }

    let content = fs::read_file(&fleet_nix)?;
    let host_map = nix::extract_host_services(&content, &fleet_nix)?;

    for (host, services) in host_map {
        entries.push(HostInfo {
            name: host,
            environment: env_name.to_string(),
            services,
        });
    }

    Ok(())
}
