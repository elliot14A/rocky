use std::collections::HashMap;
use std::path::Path;

use crate::commands::env::list::collect_environments;
use crate::commands::host::list::collect_hosts;
use crate::commands::secret::list::collect_secrets;
pub use crate::commands::secret::list::SecretInfo as Secret;
use crate::commands::service::list::collect_services;
use crate::error::Result;
use crate::nix;
use crate::process::mesh;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Service {
    pub name: String,
    pub kind: String,
    pub target: String,
    pub replicas: u32,
    pub base_port: Option<u16>,
    pub environments: Vec<String>,
    pub hosts: Vec<String>,
    pub secrets: Vec<String>,
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
    pub domain: String,
    pub hosts: Vec<Host>,
    pub secrets: Vec<Secret>,
    pub mesh_connected: bool,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Fleet {
    pub environments: Vec<Environment>,
    pub services: Vec<Service>,
}

#[derive(Default)]
struct AggregatedServiceConfig {
    replicas: u32,
    base_port: Option<u16>,
    environments: Vec<String>,
    hosts: Vec<String>,
}

pub async fn load_fleet(fleet_root: &Path) -> Result<Fleet> {
    let raw_envs = collect_environments(fleet_root)?;
    let raw_hosts = collect_hosts(fleet_root, None)?;
    let raw_services = collect_services(fleet_root)?;
    let raw_secrets = collect_secrets(fleet_root, None).await?;

    let mut secrets_by_env: HashMap<String, Vec<Secret>> = HashMap::new();
    for secret in raw_secrets {
        secrets_by_env
            .entry(secret.environment.clone())
            .or_default()
            .push(secret);
    }

    let mut hosts_by_env: HashMap<String, Vec<Host>> = HashMap::new();
    for host_info in raw_hosts {
        let env_dir = fleet_root.join("environments").join(&host_info.environment);
        let host_dir = env_dir.join("hosts").join(&host_info.name);

        let has_disko = host_dir.join("disko.nix").is_file();
        let has_hardware = host_dir.join("hardware.nix").is_file();

        let has_key = nix::diff_env(&env_dir)
            .map(|d| d.installed.contains(&host_info.name))
            .unwrap_or(false);

        let magic_dns = format!("{}-{}", host_info.environment, host_info.name);

        hosts_by_env
            .entry(host_info.environment.clone())
            .or_default()
            .push(Host {
                name: host_info.name,
                environment: host_info.environment,
                services: host_info.services,
                magic_dns,
                has_disko,
                has_hardware,
                has_key,
            });
    }

    let mut environments = Vec::new();
    let mut svc_configs: HashMap<String, AggregatedServiceConfig> = HashMap::new();

    for env_info in raw_envs {
        let env_name = env_info.name;
        let domain = mesh::resolve_domain(fleet_root, &env_name)
            .await
            .unwrap_or_else(|_| format!("{env_name}.rocky.internal"));

        let (mesh_connected, _) = mesh::check_workstation_tailscale(&domain, &env_name)
            .await
            .unwrap_or((false, None));

        let env_dir = fleet_root.join("environments").join(&env_name);
        let fleet_nix_file = env_dir.join("fleet.nix");
        if fleet_nix_file.is_file() {
            if let Ok(content) = crate::fs::read_file(&fleet_nix_file) {
                if let Ok(host_map) = nix::extract_host_services_config(&content, &fleet_nix_file) {
                    for (host_name, configs) in host_map {
                        for cfg in configs {
                            let entry = svc_configs.entry(cfg.name).or_insert_with(|| {
                                AggregatedServiceConfig {
                                    replicas: 1,
                                    base_port: None,
                                    environments: Vec::new(),
                                    hosts: Vec::new(),
                                }
                            });
                            if cfg.replicas > 1 || entry.replicas == 1 {
                                entry.replicas = cfg.replicas;
                            }
                            if cfg.base_port.is_some() {
                                entry.base_port = cfg.base_port;
                            }
                            if !entry.environments.contains(&env_name) {
                                entry.environments.push(env_name.clone());
                            }
                            let host_entry = format!("{env_name}:{host_name}");
                            if !entry.hosts.contains(&host_entry) {
                                entry.hosts.push(host_entry);
                            }
                        }
                    }
                }
            }
        }

        let hosts = hosts_by_env.remove(&env_name).unwrap_or_default();
        let secrets = secrets_by_env.remove(&env_name).unwrap_or_default();

        environments.push(Environment {
            name: env_name,
            domain,
            hosts,
            secrets,
            mesh_connected,
        });
    }

    let mut svc_secrets: HashMap<String, Vec<String>> = HashMap::new();
    for env in &environments {
        for secret in &env.secrets {
            for svc_name in &secret.services {
                let list = svc_secrets.entry(svc_name.clone()).or_default();
                if !list.contains(&secret.name) {
                    list.push(secret.name.clone());
                }
            }
        }
    }

    let services = raw_services
        .into_iter()
        .map(|s| {
            let cfg = svc_configs.remove(&s.name).unwrap_or_default();
            let secrets = svc_secrets.remove(&s.name).unwrap_or_default();
            Service {
                name: s.name,
                kind: s.kind,
                target: s.target,
                replicas: if cfg.replicas == 0 { 1 } else { cfg.replicas },
                base_port: cfg.base_port,
                environments: cfg.environments,
                hosts: cfg.hosts,
                secrets,
            }
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
        let examples_dir = Path::new(env!("CARGO_MANIFEST_DIR")).join("examples/supabase");
        let fleet = load_fleet(&examples_dir)
            .await
            .expect("failed to load examples/supabase");

        assert_eq!(fleet.environments.len(), 1);
        let prod = &fleet.environments[0];
        assert_eq!(prod.name, "prod");
        assert_eq!(prod.hosts.len(), 3);

        let host_names: Vec<&str> = prod.hosts.iter().map(|h| h.name.as_str()).collect();
        assert_eq!(host_names, vec!["app", "db", "edge"]);

        let app_host = prod.hosts.iter().find(|h| h.name == "app").unwrap();
        assert_eq!(
            app_host.services,
            vec![
                "postgrest".to_string(),
                "gotrue".to_string(),
                "storage".to_string()
            ]
        );
        assert!(app_host.has_disko);
        assert!(app_host.has_hardware);

        assert_eq!(fleet.services.len(), 8);
        assert_eq!(prod.secrets.len(), 5);

        let gotrue = fleet.services.iter().find(|s| s.name == "gotrue").unwrap();
        assert_eq!(gotrue.replicas, 2);
        assert_eq!(gotrue.base_port, Some(9999));
        assert!(gotrue.environments.contains(&"prod".to_string()));
        assert!(gotrue.secrets.contains(&"jwt.age".to_string()));
        assert!(gotrue.secrets.contains(&"postgres-creds.age".to_string()));

        let jwt = prod.secrets.iter().find(|s| s.name == "jwt.age").unwrap();
        assert!(jwt.services.contains(&"postgrest".to_string()));
        assert!(jwt.services.contains(&"gotrue".to_string()));
    }
}
