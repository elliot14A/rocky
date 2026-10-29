use std::collections::HashMap;
use std::path::{Path, PathBuf};
use std::process::Stdio;
use std::sync::Arc;

use snafu::ResultExt;
use tap::prelude::*;
use tokio::process::Command;
use tokio::sync::Semaphore;
use tokio::task::JoinSet;

use crate::error::{
    AlreadyExistsSnafu, CommandExecutionSnafu, CommandSignaledSnafu, Kind, ProcessSpawnSnafu,
    Result,
};

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct HostDeploy {
    pub name: String,
    pub flake_attr: Option<String>,
    pub hostname: Option<String>,
    pub port: Option<u16>,
}

#[derive(Debug)]
pub struct DeployResult {
    pub name: String,
    pub status: Result<()>,
    pub log_path: PathBuf,
}

pub fn build_deploy_args(
    host: &HostDeploy,
    fleet_root: &Path,
    identity: Option<&Path>,
) -> Vec<String> {
    let mut args = Vec::new();

    let target_attr = host.flake_attr.as_deref().unwrap_or(&host.name);
    let flake_target = format!("{}#{}", fleet_root.display(), target_attr);
    args.push(flake_target);

    if let Some(ref hostname) = host.hostname {
        args.push("--hostname".to_string());
        args.push(hostname.clone());
    }

    let mut ssh_opts = Vec::new();

    if let Some(port) = host.port {
        if port != 22 {
            ssh_opts.push(format!("-p {port}"));
        }
    }

    if let Some(id) = identity {
        ssh_opts.push(format!("-i \"{}\"", id.display()));
    }

    if !ssh_opts.is_empty() {
        args.push("--ssh-opts".to_string());
        args.push(ssh_opts.join(" "));
    }

    args
}

pub async fn deploy_host(
    host: &HostDeploy,
    fleet_root: &Path,
    identity: Option<&Path>,
    log_path: &Path,
) -> Result<()> {
    crate::validate::validate_name(Kind::Host, &host.name)?;

    let abs_identity = crate::ssh::canonicalize_identity(identity)?;
    let (out_file, err_file) = crate::process::open_log_pair(log_path)?;

    let args = build_deploy_args(host, fleet_root, abs_identity.as_deref());

    let mut command = Command::new("deploy");
    command.args(&args);
    command.stdin(Stdio::null());
    command.stdout(Stdio::from(out_file));
    command.stderr(Stdio::from(err_file));
    command.kill_on_drop(true);

    let mut child = command
        .spawn()
        .context(ProcessSpawnSnafu { command: "deploy" })?;

    let exit_status = child
        .wait()
        .await
        .context(ProcessSpawnSnafu { command: "deploy" })?;

    if !exit_status.success() {
        let display_cmd = format!("deploy {}", args.join(" "));
        let tail_log = crate::process::tail_log(log_path, 10);

        return match exit_status.code() {
            Some(status) => CommandExecutionSnafu {
                command: display_cmd,
                status,
                stderr: tail_log,
                log_path: Some(log_path.to_path_buf()),
            }
            .fail(),
            None => CommandSignaledSnafu {
                command: display_cmd,
            }
            .fail(),
        };
    }

    Ok(()).tap_ok(|_| {
        tracing::info!(
            host = %host.name,
            log = %log_path.display(),
            "deploy completed"
        );
    })
}

pub async fn deploy_single(
    target: &str,
    fleet_root: &Path,
    hostname: Option<&str>,
    identity: Option<&Path>,
) -> Result<()> {
    let host = HostDeploy {
        name: target.to_string(),
        flake_attr: None,
        hostname: hostname.map(ToString::to_string),
        port: None,
    };
    let log_path = crate::process::log::host_log_path("deploy", target)?;
    deploy_host(&host, fleet_root, identity, &log_path).await
}

pub async fn deploy_fleet(
    hosts: &[HostDeploy],
    fleet_root: &Path,
    identity: Option<&Path>,
    parallel_limit: usize,
) -> Vec<DeployResult> {
    if hosts.is_empty() {
        return Vec::new();
    }

    let limit = if parallel_limit == 0 {
        4
    } else {
        parallel_limit
    };
    let semaphore = Arc::new(Semaphore::new(limit));
    let mut join_set = JoinSet::new();
    let mut host_lookup: HashMap<tokio::task::Id, (String, PathBuf)> = HashMap::new();

    let mut early_results = Vec::new();
    let mut seen_hosts = std::collections::HashSet::new();

    for host in hosts {
        if !seen_hosts.insert(&host.name) {
            early_results.push(DeployResult {
                name: host.name.clone(),
                status: AlreadyExistsSnafu {
                    kind: Kind::Host,
                    name: host.name.clone(),
                    path: PathBuf::new(),
                }
                .fail(),
                log_path: PathBuf::new(),
            });
            continue;
        }

        let log_path = match crate::process::log::host_log_path("deploy", &host.name) {
            Ok(p) => p,
            Err(e) => {
                early_results.push(DeployResult {
                    name: host.name.clone(),
                    status: Err(e),
                    log_path: PathBuf::new(),
                });
                continue;
            }
        };

        let host_cloned = host.clone();
        let fleet_root_cloned = fleet_root.to_path_buf();
        let identity_cloned = identity.map(Path::to_path_buf);
        let sem = Arc::clone(&semaphore);
        let log_cloned = log_path.clone();

        let abort_handle = join_set.spawn(async move {
            let permit = sem.acquire_owned().await;
            let status = match permit {
                Ok(_permit) => {
                    deploy_host(
                        &host_cloned,
                        &fleet_root_cloned,
                        identity_cloned.as_deref(),
                        &log_cloned,
                    )
                    .await
                }
                Err(_) => CommandSignaledSnafu {
                    command: format!("deploy {}", host_cloned.name),
                }
                .fail(),
            };

            DeployResult {
                name: host_cloned.name,
                status,
                log_path: log_cloned,
            }
        });

        host_lookup.insert(abort_handle.id(), (host.name.clone(), log_path));
    }

    let mut results = early_results;

    loop {
        tokio::select! {
            biased;
            _ = tokio::signal::ctrl_c() => {
                tracing::warn!("Deployment cancelled by operator (Ctrl+C). Terminating running tasks.");
                join_set.abort_all();
                while let Some(join_res) = join_set.join_next().await {
                    match join_res {
                        Ok(res) => results.push(res),
                        Err(join_err) => {
                            if let Some((name, log_path)) = host_lookup.get(&join_err.id()) {
                                results.push(DeployResult {
                                    name: name.clone(),
                                    status: CommandSignaledSnafu {
                                        command: format!("deploy on '{name}' (cancelled)"),
                                    }.fail(),
                                    log_path: log_path.clone(),
                                });
                            }
                        }
                    }
                }
                break;
            }
            maybe_res = join_set.join_next() => {
                match maybe_res {
                    Some(Ok(res)) => {
                        results.push(res);
                    }
                    Some(Err(join_err)) => {
                        if let Some((name, log_path)) = host_lookup.get(&join_err.id()) {
                            results.push(DeployResult {
                                name: name.clone(),
                                status: CommandSignaledSnafu {
                                    command: format!("deploy on '{name}'"),
                                }.fail(),
                                log_path: log_path.clone(),
                            });
                        }
                    }
                    None => break,
                }
            }
        }
    }

    results
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_build_deploy_args_default() {
        let host = HostDeploy {
            name: "web-01".to_string(),
            flake_attr: None,
            hostname: None,
            port: None,
        };

        let args = build_deploy_args(&host, Path::new("."), None);
        assert_eq!(args, vec![".#web-01"]);
    }

    #[test]
    fn test_build_deploy_args_with_custom_flake_attr() {
        let host = HostDeploy {
            name: "edge".to_string(),
            flake_attr: Some("prod-edge".to_string()),
            hostname: None,
            port: None,
        };

        let args = build_deploy_args(&host, Path::new("/fleet"), None);
        assert_eq!(args, vec!["/fleet#prod-edge"]);
    }

    #[test]
    fn test_build_deploy_args_with_hostname_and_identity() {
        let host = HostDeploy {
            name: "db-01".to_string(),
            flake_attr: None,
            hostname: Some("192.168.1.100".to_string()),
            port: None,
        };
        let identity = Path::new("/keys/id_ed25519");

        let args = build_deploy_args(&host, Path::new("/fleet"), Some(identity));
        assert_eq!(
            args,
            vec![
                "/fleet#db-01",
                "--hostname",
                "192.168.1.100",
                "--ssh-opts",
                "-i \"/keys/id_ed25519\""
            ]
        );
    }

    #[test]
    fn test_build_deploy_args_with_custom_port() {
        let host = HostDeploy {
            name: "edge-01".to_string(),
            flake_attr: None,
            hostname: Some("10.0.0.1".to_string()),
            port: Some(2222),
        };

        let args = build_deploy_args(&host, Path::new("/fleet"), None);
        assert_eq!(
            args,
            vec![
                "/fleet#edge-01",
                "--hostname",
                "10.0.0.1",
                "--ssh-opts",
                "-p 2222"
            ]
        );
    }

    #[tokio::test]
    async fn test_deploy_host_rejects_missing_identity() {
        let host = HostDeploy {
            name: "web-01".to_string(),
            flake_attr: None,
            hostname: None,
            port: None,
        };
        let missing_key = Path::new("/nonexistent/key");
        let log = Path::new("/tmp/deploy-test.log");

        let result = deploy_host(&host, Path::new("."), Some(missing_key), log).await;
        assert!(result.is_err());
    }

    #[tokio::test]
    async fn test_deploy_fleet_empty_hosts() {
        let results = deploy_fleet(&[], Path::new("."), None, 4).await;
        assert!(results.is_empty());
    }
}
