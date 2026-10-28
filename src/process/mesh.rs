use std::collections::HashMap;
use std::path::{Path, PathBuf};

use serde::{Deserialize, Serialize};

use crate::error::{MeshSnafu, Result};
use crate::process;

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct FleetHostStatus {
    pub name: String,
    pub status: String,
    pub mesh_ip: String,
    pub dns_name: String,
    pub last_seen: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct FleetMeshStatus {
    pub environment: String,
    pub login_server: String,
    pub domain: String,
    pub workstation_connected: bool,
    pub workstation_ip: Option<String>,
    pub hosts: Vec<FleetHostStatus>,
}

#[derive(Debug, Clone)]
pub struct ControlPlaneNode {
    pub name: String,
    pub ip: String,
    pub online: bool,
    pub last_seen: String,
}

#[derive(Deserialize)]
struct RawHeadscaleNode {
    name: Option<String>,
    given_name: Option<String>,
    ip_addresses: Option<Vec<String>>,
    online: Option<bool>,
    last_seen: Option<RawHeadscaleTimestamp>,
}

#[derive(Deserialize)]
struct RawHeadscaleTimestamp {
    seconds: Option<i64>,
}

#[derive(Deserialize)]
struct RawTailscaleStatus {
    #[serde(rename = "BackendState")]
    backend_state: Option<String>,
    #[serde(rename = "CurrentTailnet")]
    current_tailnet: Option<RawTailnet>,
    #[serde(rename = "MagicDNSSuffix")]
    magic_dns_suffix: Option<String>,
    #[serde(rename = "Self")]
    self_node: Option<RawNode>,
    #[serde(rename = "Peer")]
    peers: Option<HashMap<String, RawPeerNode>>,
    #[serde(rename = "User")]
    users: Option<HashMap<String, RawTailscaleUser>>,
}

#[derive(Deserialize)]
struct RawTailnet {
    #[serde(rename = "Name")]
    name: Option<String>,
    #[serde(rename = "MagicDNSSuffix")]
    magic_dns_suffix: Option<String>,
}

#[derive(Deserialize)]
struct RawTailscaleUser {
    #[serde(rename = "LoginName")]
    login_name: Option<String>,
}

#[derive(Deserialize)]
struct RawPeerNode {
    #[serde(rename = "HostName")]
    hostname: Option<String>,
}

#[derive(Deserialize)]
struct RawNode {
    #[serde(rename = "TailscaleIPs")]
    ips: Option<Vec<String>>,
    #[serde(rename = "UserID")]
    user_id: Option<serde_json::Value>,
}

pub fn resolve_headscale_bin() -> Result<String> {
    if let Ok(path) = which_bin("headscale") {
        return Ok(path);
    }
    MeshSnafu {
        reason: "Headscale binary 'headscale' not found in PATH.",
    }
    .fail()
}

fn which_bin(name: &str) -> std::result::Result<String, ()> {
    if let Ok(path) = std::env::var("PATH") {
        for dir in std::env::split_paths(&path) {
            let candidate = dir.join(name);
            if candidate.is_file() {
                return Ok(candidate.to_string_lossy().to_string());
            }
        }
    }
    Err(())
}

pub fn format_last_seen(secs_opt: Option<i64>) -> String {
    let Some(secs) = secs_opt else {
        return "-".to_string();
    };
    if secs <= 0 {
        return "-".to_string();
    }
    let now = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|d| d.as_secs() as i64)
        .unwrap_or(secs);

    let diff = now.saturating_sub(secs);
    if diff < 10 {
        "just now".to_string()
    } else if diff < 60 {
        format!("{diff}s ago")
    } else if diff < 3600 {
        format!("{}m ago", diff / 60)
    } else if diff < 86400 {
        format!("{}h ago", diff / 3600)
    } else {
        format!("{}d ago", diff / 86400)
    }
}

pub fn parse_headscale_nodes_json(json_str: &str) -> Vec<ControlPlaneNode> {
    let raw: Vec<RawHeadscaleNode> = match serde_json::from_str(json_str) {
        Ok(v) => v,
        Err(_) => return Vec::new(),
    };

    raw.into_iter()
        .filter_map(|node| {
            let name = node.given_name.or(node.name)?;
            let ip = node
                .ip_addresses
                .and_then(|ips| {
                    ips.into_iter()
                        .find(|ip| ip.contains('.') && !ip.contains(':'))
                })
                .unwrap_or_else(|| "-".to_string());
            let online = node.online.unwrap_or(false);
            let last_seen = format_last_seen(node.last_seen.and_then(|t| t.seconds));

            Some(ControlPlaneNode {
                name,
                ip,
                online,
                last_seen,
            })
        })
        .collect()
}

pub async fn query_headscale_nodes(config_path: &Path) -> Result<HashMap<String, ControlPlaneNode>> {
    let bin = resolve_headscale_bin()?;
    let args = [
        "--config",
        &config_path.to_string_lossy(),
        "nodes",
        "list",
        "-o",
        "json",
    ];

    let output = process::exec(&bin, args, None).await?;
    let mut map = HashMap::new();
    for node in parse_headscale_nodes_json(&output) {
        map.insert(node.name.clone(), node);
    }
    Ok(map)
}

pub async fn check_workstation_tailscale(
    expected_domain: &str,
    env: &str,
) -> Result<(bool, Option<String>)> {
    let output = match process::exec("tailscale", ["status", "--json"], None).await {
        Ok(out) => out,
        Err(_) => return Ok((false, None)),
    };

    let raw: RawTailscaleStatus = match serde_json::from_str(&output) {
        Ok(r) => r,
        Err(_) => return Ok((false, None)),
    };

    let running = raw
        .backend_state
        .as_deref()
        .map(|s| s.eq_ignore_ascii_case("Running"))
        .unwrap_or(false);

    if !running {
        return Ok((false, None));
    }

    let tailnet_name = raw
        .current_tailnet
        .as_ref()
        .and_then(|t| t.name.as_deref())
        .unwrap_or_default();
    let magic_dns = raw
        .magic_dns_suffix
        .as_deref()
        .or_else(|| {
            raw.current_tailnet
                .as_ref()
                .and_then(|t| t.magic_dns_suffix.as_deref())
        })
        .unwrap_or_default();

    let current_user_login = raw
        .self_node
        .as_ref()
        .and_then(|n| n.user_id.as_ref())
        .and_then(|uid| {
            let key = match uid {
                serde_json::Value::Number(num) => num.to_string(),
                serde_json::Value::String(s) => s.clone(),
                _ => return None,
            };
            raw.users.as_ref()?.get(&key)?.login_name.as_deref()
        });

    let matches_env_user = current_user_login
        .map(|u| u.eq_ignore_ascii_case(env))
        .unwrap_or(false);

    let prefix = format!("{env}-");
    let has_env_peer = raw
        .peers
        .as_ref()
        .map(|p| {
            p.values().any(|node| {
                node.hostname
                    .as_deref()
                    .map(|h| h.starts_with(&prefix))
                    .unwrap_or(false)
            })
        })
        .unwrap_or(false);

    let matches_env_domain = (!magic_dns.is_empty()
        && (magic_dns == expected_domain || magic_dns.starts_with(&format!("{env}."))))
        || (!tailnet_name.is_empty()
            && (tailnet_name.eq_ignore_ascii_case(env)
                || tailnet_name == expected_domain
                || tailnet_name.starts_with(&format!("{env}."))));

    let is_connected = has_env_peer || matches_env_user || matches_env_domain;

    if is_connected {
        let ip = raw
            .self_node
            .and_then(|n| n.ips)
            .and_then(|v| v.into_iter().find(|ip| ip.contains('.')));
        Ok((true, ip))
    } else {
        Ok((false, None))
    }
}

pub async fn resolve_domain(fleet_root: &Path, env: &str) -> Result<String> {
    crate::utils::validate::validate_name(crate::error::Kind::Environment, env)?;
    let fleet_nix = fleet_root.join("environments").join(env).join("fleet.nix");
    let expr = format!(
        "let f = import (toString \"{}\"); in f.settings.domain or \"{}.rocky.internal\"",
        fleet_nix.display(),
        env
    );

    let raw = crate::nix::eval(&expr).await?;
    let trimmed = raw.trim().trim_matches('"').trim();

    let domain = if trimmed == "null" || trimmed.is_empty() {
        format!("{env}.rocky.internal")
    } else {
        trimmed.to_string()
    };

    Ok(domain)
}

pub async fn resolve_login_server(fleet_root: &Path, env: &str) -> Result<String> {
    crate::utils::validate::validate_name(crate::error::Kind::Environment, env)?;
    let fleet_nix = fleet_root.join("environments").join(env).join("fleet.nix");
    let expr = format!(
        "let f = import (toString \"{}\"); in f.settings.tailscaleLoginServer or null",
        fleet_nix.display()
    );

    let raw = crate::nix::eval(&expr).await?;
    let trimmed = raw.trim().trim_matches('"').trim();

    let server = if trimmed == "null" || trimmed.is_empty() {
        "https://controlplane.tailscale.com".to_string()
    } else if trimmed.contains("10.0.2.2") {
        trimmed.replace("10.0.2.2", "127.0.0.1")
    } else {
        trimmed.to_string()
    };

    Ok(server)
}

pub async fn resolve_authkey(
    fleet_root: &Path,
    env: &str,
    explicit_key: Option<&str>,
    identity_path: Option<&Path>,
) -> Result<String> {
    crate::utils::validate::validate_name(crate::error::Kind::Environment, env)?;
    if let Some(key) = explicit_key {
        let trimmed = key.trim();
        if !trimmed.is_empty() {
            return Ok(trimmed.to_string());
        }
    }

    let env_dir = fleet_root.join("environments").join(env);
    if !env_dir.is_dir() {
        return MeshSnafu {
            reason: format!("Environment directory not found at '{}'", env_dir.display()),
        }
        .fail();
    }

    let secrets_dir = crate::nix::resolve_secrets_dir(&env_dir);
    let secret_path = secrets_dir.join("tailscale-authkey.age");
    if !secret_path.is_file() {
        return MeshSnafu {
            reason: format!(
                "Tailscale authkey secret not found in environment '{env}'. Expected '{}' to exist or pass --authkey <key>.",
                secret_path.display()
            ),
        }
        .fail();
    }

    let ciphertext = crate::fs::read_file_bytes(&secret_path)?;
    let decrypted = crate::crypto::age::decrypt(&ciphertext, identity_path).await?;
    let content = String::from_utf8(decrypted).map_err(|e| {
        MeshSnafu {
            reason: format!(
                "Decrypted authkey in '{}' is not valid UTF-8: {e}",
                secret_path.display()
            ),
        }
        .build()
    })?;

    let trimmed = content.trim();
    if trimmed.is_empty() {
        return MeshSnafu {
            reason: format!("Secret '{}' is empty", secret_path.display()),
        }
        .fail();
    }

    if crate::crypto::secret::format::has_key_value_entries(trimmed) {
        return MeshSnafu {
            reason: format!(
                "Secret '{}' contains key-value pairs. Tailscale authkey secret must be a raw token, not key-value format.",
                secret_path.display()
            ),
        }
        .fail();
    }

    Ok(trimmed.to_string())
}

pub async fn get_fleet_mesh_status(fleet_root: &Path, env: &str) -> Result<FleetMeshStatus> {
    crate::utils::validate::validate_name(crate::error::Kind::Environment, env)?;
    let fleet_nix = fleet_root.join("environments").join(env).join("fleet.nix");
    let content = crate::fs::read_file(&fleet_nix)?;
    let host_map = crate::nix::extract_host_services(&content, &fleet_nix)?;
    let mut declared_hosts: Vec<String> = host_map.into_keys().collect();
    declared_hosts.sort();

    let server = resolve_login_server(fleet_root, env).await?;
    let domain = resolve_domain(fleet_root, env).await?;

    let (workstation_connected, workstation_ip) =
        check_workstation_tailscale(&domain, env).await?;

    let mut node_map = HashMap::new();
    if server.contains("127.0.0.1") || server.contains("localhost") {
        let home = std::env::var("HOME").unwrap_or_default();
        let headscale_cfg = PathBuf::from(home).join(".local/state/rocky/headscale/config.yaml");
        if headscale_cfg.is_file() {
            node_map = query_headscale_nodes(&headscale_cfg).await?;
        }
    }

    let mut hosts = Vec::new();
    for host in declared_hosts {
        let node_name = format!("{env}-{host}");
        let dns_name = node_name.clone();
        if let Some(node) = node_map.get(&node_name) {
            let status = if node.online {
                "Online".to_string()
            } else {
                "Offline".to_string()
            };
            hosts.push(FleetHostStatus {
                name: node_name,
                status,
                mesh_ip: node.ip.clone(),
                dns_name,
                last_seen: node.last_seen.clone(),
            });
        } else {
            hosts.push(FleetHostStatus {
                name: node_name,
                status: "Not registered".to_string(),
                mesh_ip: "-".to_string(),
                dns_name,
                last_seen: "-".to_string(),
            });
        }
    }

    Ok(FleetMeshStatus {
        environment: env.to_string(),
        login_server: server,
        domain,
        workstation_connected,
        workstation_ip,
        hosts,
    })
}

pub async fn mesh_join(server: &str, authkey: &str, force_reauth: bool) -> Result<()> {
    let login_arg = format!("--login-server={server}");
    let authkey_arg = format!("--authkey={authkey}");
    let mut up_args = vec![
        "up",
        &login_arg,
        &authkey_arg,
        "--accept-dns=true",
    ];

    if force_reauth {
        up_args.push("--force-reauth");
    }

    let res = process::exec("tailscale", up_args, None).await;
    match res {
        Ok(_) => Ok(()),
        Err(e) => {
            let msg = e.to_string();
            if msg.contains("can't change --login-server without --force-reauth") {
                MeshSnafu {
                    reason: "Tailscale requires re-authentication to switch login servers. Run again with: rocky mesh join --force-reauth",
                }
                .fail()
            } else if msg.contains("Access denied") {
                MeshSnafu {
                    reason: "Tailscaled denied access. Run 'sudo tailscale set --operator=$USER' once to manage Tailscale without root, or run this command with sudo.",
                }
                .fail()
            } else {
                Err(e)
            }
        }
    }
}

pub async fn mesh_leave(profile_to_restore: Option<&str>) -> Result<()> {
    let res = if let Some(target) = profile_to_restore {
        process::exec("tailscale", ["switch", target], None).await
    } else {
        process::exec("tailscale", ["down"], None).await
    };

    match res {
        Ok(_) => Ok(()),
        Err(e) => {
            let msg = e.to_string();
            if msg.contains("Access denied") {
                MeshSnafu {
                    reason: "Tailscaled denied access. Run 'sudo tailscale set --operator=$USER' once to manage Tailscale without root, or run this command with sudo.",
                }
                .fail()
            } else {
                Err(e)
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_parse_headscale_nodes_json() {
        let json = r#"[
            {
                "id": 1,
                "name": "prod-db",
                "given_name": "prod-db",
                "ip_addresses": ["100.64.0.2", "fd7a:115c:a1e0::2"],
                "online": true,
                "last_seen": { "seconds": 1792825266 }
            },
            {
                "id": 2,
                "name": "prod-edge",
                "given_name": "prod-edge",
                "ip_addresses": ["100.64.0.1"],
                "online": false,
                "last_seen": null
            }
        ]"#;

        let nodes = parse_headscale_nodes_json(json);
        assert_eq!(nodes.len(), 2);
        assert_eq!(nodes[0].name, "prod-db");
        assert_eq!(nodes[0].ip, "100.64.0.2");
        assert!(nodes[0].online);
        assert_eq!(nodes[1].name, "prod-edge");
        assert_eq!(nodes[1].ip, "100.64.0.1");
        assert!(!nodes[1].online);
        assert_eq!(nodes[1].last_seen, "-");
    }

    #[test]
    fn test_format_last_seen_none() {
        assert_eq!(format_last_seen(None), "-");
    }

    #[test]
    fn test_format_last_seen_recent() {
        let now = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_secs() as i64;
        assert_eq!(format_last_seen(Some(now - 2)), "just now");
        assert_eq!(format_last_seen(Some(now - 25)), "25s ago");
    }
}
