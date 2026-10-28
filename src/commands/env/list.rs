use std::path::Path;

use clap::Args;

use crate::error::Result;
use crate::{fs, nix, ui};

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct EnvInfo {
    pub name: String,
    pub host_count: usize,
    pub secret_count: usize,
}

#[derive(Args, Debug, Clone, Default)]
pub struct ListArgs {}

pub async fn run(args: ListArgs, target: Option<&Path>) -> Result<()> {
    nix::with_fleet(target, |fleet_root| async move {
        execute(args, &fleet_root).await
    })
    .await
}

pub fn collect_environments(fleet_root: &Path) -> Result<Vec<EnvInfo>> {
    let env_dirs = nix::list_environments(fleet_root)?;
    let mut environments = Vec::new();

    for (env_name, env_dir) in env_dirs {
        let fleet_nix = env_dir.join("fleet.nix");
        if !fleet_nix.is_file() {
            continue;
        }

        let host_count = match fs::read_file(&fleet_nix) {
            Ok(content) => nix::extract_host_services(&content, &fleet_nix)
                .map(|hosts| hosts.len())
                .unwrap_or(0),
            Err(_) => 0,
        };

        let secrets_dir = nix::resolve_secrets_dir(&env_dir);
        let secret_count = std::fs::read_dir(&secrets_dir)
            .map(|entries| {
                entries
                    .flatten()
                    .filter(|e| e.path().extension().and_then(|ext| ext.to_str()) == Some("age"))
                    .count()
            })
            .unwrap_or(0);

        environments.push(EnvInfo {
            name: env_name,
            host_count,
            secret_count,
        });
    }

    environments.sort_by(|a, b| a.name.cmp(&b.name));
    Ok(environments)
}

async fn execute(_args: ListArgs, fleet_root: &Path) -> Result<()> {
    let environments = collect_environments(fleet_root)?;

    if environments.is_empty() {
        ui::warn("No environments found in fleet");
        return Ok(());
    }

    println!("{:<16} {:<10} {:<10}", "ENVIRONMENT", "HOSTS", "SECRETS");
    println!("{:-<16} {:-<10} {:-<10}", "", "", "");

    for env in environments {
        println!("{:<16} {:<10} {:<10}", env.name, env.host_count, env.secret_count);
    }

    Ok(())
}
