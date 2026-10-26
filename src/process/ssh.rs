use std::path::{Path, PathBuf};

use snafu::{IntoError, OptionExt};

use crate::error::{Kind, MeshSnafu, NotFoundSnafu, ProcessSpawnSnafu, Result, TargetParseSnafu};
use crate::process;
use crate::process::mesh;

pub fn parse_ssh_target(fleet_root: &Path, raw: &str) -> Result<(String, String)> {
    let envs_dir = fleet_root.join("environments");
    if let Ok(entries) = std::fs::read_dir(&envs_dir) {
        let mut env_names: Vec<String> = entries
            .flatten()
            .filter(|e| e.path().is_dir())
            .filter_map(|e| e.file_name().into_string().ok())
            .filter(|n| !n.starts_with('.'))
            .collect();
        env_names.sort_by_key(|b| std::cmp::Reverse(b.len()));

        for env in env_names {
            let prefix = format!("{env}-");
            if let Some(host) = raw.strip_prefix(&prefix) {
                if !host.is_empty() {
                    return Ok((env, host.to_string()));
                }
            }
        }
    }

    if let Some((env, host)) = raw.split_once('-') {
        if !env.is_empty() && !host.is_empty() {
            return Ok((env.to_string(), host.to_string()));
        }
    }

    TargetParseSnafu {
        raw: raw.to_string(),
        reason: "expected target in <env>-<host> format (e.g. prod-db)".to_string(),
    }
    .fail()
}

pub use parse_ssh_target as parse_target;

pub fn resolve_identity_key() -> Option<PathBuf> {
    if let Ok(key) = std::env::var("ROCKY_SSH_KEY") {
        let p = PathBuf::from(key.trim());
        if p.is_file() {
            return Some(p);
        }
    }

    let home = std::env::var("HOME").ok().map(PathBuf::from)?;
    let ssh_dir = home.join(".ssh");

    let candidates = [
        "agenix_ed25519",
        "id_ed25519",
        "id_ecdsa",
        "id_rsa",
    ];

    for candidate in candidates {
        let p = ssh_dir.join(candidate);
        if p.is_file() {
            return Some(p);
        }
    }

    None
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
    let base_domain = domain.trim_start_matches(|c| c != '.').trim_start_matches('.');
    if !base_domain.is_empty() && !target.ends_with(base_domain) {
        format!("{target}.{base_domain}")
    } else {
        format!("{target}.{domain}")
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
    }
    args.push("-o".to_string());
    args.push("ForwardAgent=no".to_string());
    args.push("-o".to_string());
    args.push("StrictHostKeyChecking=accept-new".to_string());
    args.push(format!("{user}@{target_addr}"));
    args.extend(extra_args.iter().cloned());
    args
}

pub async fn execute_ssh(
    fleet_root: &Path,
    target_raw: &str,
    extra_args: &[String],
) -> Result<()> {
    let (env, host) = parse_target(fleet_root, target_raw)?;

    let fleet_nix = fleet_root.join("environments").join(&env).join("fleet.nix");
    fleet_nix.is_file().then_some(()).context(NotFoundSnafu {
        kind: Kind::Environment,
        name: env.clone(),
        path: fleet_root.join("environments").join(&env),
    })?;

    let content = crate::fs::read_file(&fleet_nix)?;
    let host_map = crate::nix::extract_host_services(&content, &fleet_nix)?;
    host_map.contains_key(&host).then_some(()).context(NotFoundSnafu {
        kind: Kind::Host,
        name: host.clone(),
        path: fleet_nix.clone(),
    })?;

    let domain = mesh::resolve_domain(fleet_root, &env).await?;
    let (is_joined, _) = mesh::check_workstation_tailscale(&domain, &env).await?;
    if !is_joined {
        return MeshSnafu {
            reason: format!(
                "Workstation is not connected to fleet mesh '{env}'. Run 'rocky mesh join {env}' first."
            ),
        }
        .fail();
    }

    let resolved_addr = resolve_target_address(target_raw, &domain).await;
    let identity = resolve_identity_key();
    let user = std::env::var("ROCKY_SSH_USER").unwrap_or_else(|_| "rocky".to_string());

    let ssh_args = build_ssh_args(identity.as_deref(), &resolved_addr, &user, extra_args);
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
    use tempfile::tempdir;

    #[test]
    fn test_parse_target_valid_single_hyphen() {
        let temp = tempdir().unwrap();
        let (env, host) = parse_target(temp.path(), "prod-db").unwrap();
        assert_eq!(env, "prod");
        assert_eq!(host, "db");
    }

    #[test]
    fn test_parse_target_with_existing_environment_prefix() {
        let temp = tempdir().unwrap();
        let env_dir = temp.path().join("environments").join("staging-west");
        std::fs::create_dir_all(&env_dir).unwrap();

        let (env, host) = parse_target(temp.path(), "staging-west-api").unwrap();
        assert_eq!(env, "staging-west");
        assert_eq!(host, "api");
    }

    #[test]
    fn test_parse_target_missing_hyphen_fails() {
        let temp = tempdir().unwrap();
        let err = parse_target(temp.path(), "proddb").unwrap_err();
        assert!(err.to_string().contains("expected target in <env>-<host> format"));
    }

    #[test]
    fn test_parse_target_empty_parts_fail() {
        let temp = tempdir().unwrap();
        assert!(parse_target(temp.path(), "-db").is_err());
        assert!(parse_target(temp.path(), "prod-").is_err());
    }

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
                "ForwardAgent=no",
                "-o",
                "StrictHostKeyChecking=accept-new",
                "rocky@100.64.0.2",
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
