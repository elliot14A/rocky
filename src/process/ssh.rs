use std::path::{Path, PathBuf};

use snafu::{ensure, IntoError};

use crate::error::{Kind, MeshSnafu, NotFoundSnafu, ProcessSpawnSnafu, Result, TargetParseSnafu};
use crate::process;
use crate::process::mesh;

pub fn resolve_identity_key(explicit: Option<&Path>) -> Result<Option<PathBuf>> {
    crate::ssh::canonicalize_identity(explicit)
}

pub async fn resolve_target_address(target: &str, domain: &str) -> String {
    let query = tokio::time::timeout(
        std::time::Duration::from_secs(3),
        process::exec("tailscale", ["ip", "-4", target], None),
    )
    .await;

    if let Ok(Ok(out)) = query {
        let ip = out.trim();
        if !ip.is_empty() && ip.contains('.') && !ip.contains(':') {
            return ip.to_string();
        }
    }

    // MagicDNS fallback: if domain has an environment prefix, strip it for the MagicDNS suffix
    let domain_to_use = if let Some((env, _)) = target.split_once('-') {
        let env_prefix = format!("{env}.");
        if let Some(base) = domain.strip_prefix(&env_prefix) {
            base
        } else {
            domain
        }
    } else {
        domain
    };

    if !domain_to_use.is_empty() && !target.ends_with(domain_to_use) {
        format!("{target}.{domain_to_use}")
    } else {
        target.to_string()
    }
}

pub fn build_ssh_args(
    identity_file: Option<&Path>,
    target_addr: &str,
    user: &str,
    extra_args: &[String],
) -> Vec<String> {
    let mut args = Vec::new();
    if let Some(key) = identity_file {
        args.push("-i".to_string());
        args.push(key.to_string_lossy().to_string());
        args.push("-o".to_string());
        args.push("IdentitiesOnly=yes".to_string());
    }
    args.push("-o".to_string());
    args.push("ForwardAgent=no".to_string());
    args.push("-o".to_string());
    args.push("StrictHostKeyChecking=accept-new".to_string());
    args.push(format!("{user}@{target_addr}"));
    if !extra_args.is_empty() {
        args.push("--".to_string());
        args.extend(extra_args.iter().cloned());
    }
    args
}

pub const ROCKY_USER: &str = "rocky";

pub async fn execute_ssh(
    fleet_root: &Path,
    env: &str,
    target: &str,
    identity_arg: Option<&Path>,
    extra_args: &[String],
) -> Result<()> {
    crate::utils::validate::validate_name(Kind::Environment, env)?;
    let clean_target = target.strip_suffix(".service").unwrap_or(target);
    crate::utils::validate::validate_name(Kind::Host, clean_target)?;

    let env_dir = fleet_root.join("environments").join(env);
    ensure!(
        env_dir.is_dir(),
        NotFoundSnafu {
            kind: Kind::Environment,
            name: env.to_string(),
            path: env_dir.clone(),
        }
    );

    let fleet_nix = env_dir.join("fleet.nix");
    ensure!(
        fleet_nix.is_file(),
        NotFoundSnafu {
            kind: Kind::Environment,
            name: env.to_string(),
            path: env_dir,
        }
    );

    let content = crate::fs::read_file(&fleet_nix)?;
    let host_map = crate::nix::extract_host_services(&content, &fleet_nix)?;

    let host = if host_map.contains_key(clean_target) {
        clean_target.to_string()
    } else {
        let mut matching_hosts: Vec<String> = host_map
            .iter()
            .filter(|(_, services)| services.iter().any(|s| s == clean_target))
            .map(|(h, _)| h.clone())
            .collect();
        matching_hosts.sort();

        if matching_hosts.len() == 1 {
            matching_hosts[0].clone()
        } else if matching_hosts.is_empty() {
            return NotFoundSnafu {
                kind: Kind::Host,
                name: clean_target.to_string(),
                path: fleet_nix,
            }
            .fail();
        } else {
            return TargetParseSnafu {
                raw: clean_target.to_string(),
                reason: format!(
                    "service '{clean_target}' runs on multiple hosts ({}); specify the host directly",
                    matching_hosts.join(", ")
                ),
            }
            .fail();
        }
    };

    let domain = mesh::resolve_domain(fleet_root, env).await?;
    let (is_joined, _) = mesh::check_workstation_tailscale(&domain, env).await?;
    if !is_joined {
        return MeshSnafu {
            reason: format!(
                "Workstation is not connected to fleet mesh '{env}'. Run 'rocky mesh join {env}' first."
            ),
        }
        .fail();
    }

    let mesh_node = format!("{env}-{host}");
    let resolved_addr = resolve_target_address(&mesh_node, &domain).await;
    let identity = resolve_identity_key(identity_arg)?;
    let ssh_args = build_ssh_args(identity.as_deref(), &resolved_addr, ROCKY_USER, extra_args);
    exec_ssh(&ssh_args)
}

#[cfg(unix)]
pub fn exec_ssh(args: &[String]) -> Result<()> {
    use std::os::unix::process::CommandExt;
    use std::process::Command;

    let err = Command::new("ssh").args(args).exec();
    Err(ProcessSpawnSnafu {
        command: format!("ssh {}", args.join(" ")),
    }
    .into_error(err))
}

#[cfg(not(unix))]
pub fn exec_ssh(args: &[String]) -> Result<()> {
    use std::process::Command;

    let mut child = Command::new("ssh").args(args).spawn().map_err(|e| {
        ProcessSpawnSnafu {
            command: format!("ssh {}", args.join(" ")),
        }
        .into_error(e)
    })?;

    let status = child.wait().map_err(|e| {
        ProcessSpawnSnafu {
            command: format!("ssh {}", args.join(" ")),
        }
        .into_error(e)
    })?;

    if !status.success() {
        return crate::error::CommandExecutionSnafu {
            command: "ssh".to_string(),
            status: status.code().unwrap_or(1),
            stderr: String::new(),
            log_path: None,
        }
        .fail();
    }

    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_build_ssh_args_with_identity_and_command() {
        let key = Path::new("/home/user/.ssh/id_ed25519");
        let extra = vec!["uptime".to_string(), "-v".to_string()];
        let args = build_ssh_args(Some(key), "100.64.0.2", "rocky", &extra);

        assert_eq!(
            args,
            vec![
                "-i",
                "/home/user/.ssh/id_ed25519",
                "-o",
                "IdentitiesOnly=yes",
                "-o",
                "ForwardAgent=no",
                "-o",
                "StrictHostKeyChecking=accept-new",
                "rocky@100.64.0.2",
                "--",
                "uptime",
                "-v"
            ]
        );
    }

    #[test]
    fn test_build_ssh_args_without_identity() {
        let extra: Vec<String> = Vec::new();
        let args = build_ssh_args(None, "prod-db.rocky.internal", "rocky", &extra);

        assert_eq!(
            args,
            vec![
                "-o",
                "ForwardAgent=no",
                "-o",
                "StrictHostKeyChecking=accept-new",
                "rocky@prod-db.rocky.internal"
            ]
        );
    }
}
