use std::collections::HashMap;
use std::path::Path;

use crate::commands::env::list::collect_environments;
use crate::commands::host::list::{collect_hosts, HostInfo};
use crate::commands::secret::list::collect_secrets;
use crate::commands::service::list::collect_services;
use crate::error::Result;
use crate::fs;
use crate::process::mesh;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Service {
    pub name: String,
    pub kind: String,
    pub target: String,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Secret {
    pub name: String,
    pub environment: String,
    pub recipients: String,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Host {
    pub name: String,
    pub environment: String,
    pub services: Vec<String>,
    pub magic_dns: String,
    pub has_disko: bool,
    pub has_hardware: bool,
    pub has_key: bool,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Environment {
    pub name: String,
    pub hosts: Vec<Host>,
    pub secrets: Vec<Secret>,
    pub mesh_connected: bool,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Fleet {
    pub environments: Vec<Environment>,
    pub services: Vec<Service>,
}

pub async fn load_fleet(fleet_root: &Path) -> Result<Fleet> {
    let env_infos = collect_environments(fleet_root)?;
    let raw_hosts = collect_hosts(fleet_root, None)?;
    let raw_services = collect_services(fleet_root)?;
    let raw_secrets = collect_secrets(fleet_root, None).await?;

    let mut secrets_by_env: HashMap<String, Vec<Secret>> = HashMap::new();
    for secret in raw_secrets {
        secrets_by_env
            .entry(secret.environment.clone())
            .or_default()
            .push(Secret {
                name: secret.name,
                environment: secret.environment,
                recipients: secret.recipients,
            });
    }

    let mut hosts_by_env: HashMap<String, Vec<HostInfo>> = HashMap::new();
    for host in raw_hosts {
        hosts_by_env
            .entry(host.environment.clone())
            .or_default()
            .push(host);
    }

    let env_base = fleet_root.join("environments");
    let mut environments = Vec::new();

    for env_info in env_infos {
        let env_dir = env_base.join(&env_info.name);
        let domain = mesh::resolve_domain(fleet_root, &env_info.name)
            .await
            .unwrap_or_default();
        let mesh_connected = if domain.is_empty() {
            false
        } else {
            mesh::check_workstation_tailscale(&domain, &env_info.name)
                .await
                .map(|(joined, _)| joined)
                .unwrap_or(false)
        };

        let keys_file = env_dir.join("keys.nix");
        let keys_content = if keys_file.is_file() {
            fs::read_file(&keys_file).ok()
        } else {
            None
        };

        let env_raw_hosts = hosts_by_env.remove(&env_info.name).unwrap_or_default();
        let mut hosts = Vec::new();

        for host_info in env_raw_hosts {
            let host_dir = env_dir.join("hosts").join(&host_info.name);
            let has_disko = host_dir.join("disko.nix").is_file();
            let has_hardware = host_dir.join("hardware.nix").is_file();
            let has_key = keys_content
                .as_deref()
                .map(|c| {
                    c.contains(&format!("\"{}\"", host_info.name))
                        || c.contains(&format!("{} =", host_info.name))
                })
                .unwrap_or(false);
            let magic_dns = format!("{}-{}", env_info.name, host_info.name);

            hosts.push(Host {
                name: host_info.name,
                environment: env_info.name.clone(),
                services: host_info.services,
                magic_dns,
                has_disko,
                has_hardware,
                has_key,
            });
        }

        let secrets = secrets_by_env.remove(&env_info.name).unwrap_or_default();

        environments.push(Environment {
            name: env_info.name,
            hosts,
            secrets,
            mesh_connected,
        });
    }

    let services = raw_services
        .into_iter()
        .map(|s| Service {
            name: s.name,
            kind: s.kind,
            target: s.target,
        })
        .collect();

    Ok(Fleet {
        environments,
        services,
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn test_load_fleet_on_real_examples_supabase() {
        let manifest_dir = Path::new(env!("CARGO_MANIFEST_DIR"));
        let supabase_path = manifest_dir.join("examples/supabase");

        let fleet = load_fleet(&supabase_path)
            .await
            .expect("load_fleet failed on examples/supabase");

        assert_eq!(fleet.environments.len(), 1);
        let prod = &fleet.environments[0];
        assert_eq!(prod.name, "prod");
        assert_eq!(prod.hosts.len(), 3);

        let host_names: Vec<&str> = prod.hosts.iter().map(|h| h.name.as_str()).collect();
        assert_eq!(host_names, vec!["app", "db", "edge"]);

        let app_host = prod.hosts.iter().find(|h| h.name == "app").unwrap();
        assert!(app_host.services.contains(&"postgrest".to_string()));
        assert!(app_host.services.contains(&"gotrue".to_string()));
        assert!(app_host.services.contains(&"storage".to_string()));
        assert_eq!(app_host.magic_dns, "prod-app");

        let secret_names: Vec<&str> = prod.secrets.iter().map(|s| s.name.as_str()).collect();
        assert!(secret_names.contains(&"jwt.age"));
        assert!(secret_names.contains(&"tailscale-authkey.age"));

        assert_eq!(fleet.services.len(), 8);
    }
}
