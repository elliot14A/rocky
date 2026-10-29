use std::collections::HashMap;
use std::path::{Path, PathBuf};
use std::process::Stdio;
use std::sync::Arc;

use snafu::{ensure, ResultExt};
use tap::prelude::*;
use tokio::process::Command;
use tokio::sync::Semaphore;
use tokio::task::JoinSet;

use crate::error::{
    AlreadyExistsSnafu, CommandExecutionSnafu, CommandSignaledSnafu, IoSnafu, Kind,
    NotADirectorySnafu, NotFoundSnafu, ProcessSpawnSnafu, Result,
};

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct HostInstall {
    pub name: String,
    pub flake_attr: Option<String>,
    pub target_ip: String,
    pub ssh_port: u16,
    pub extra_files_dir: PathBuf,
}

#[derive(Debug)]
pub struct InstallResult {
    pub name: String,
    pub status: Result<()>,
    pub log_path: PathBuf,
}

pub fn build_install_args(
    host: &HostInstall,
    fleet_root: &Path,
    identity: Option<&Path>,
) -> Vec<String> {
    let mut args = Vec::new();

    let target_attr = host.flake_attr.as_deref().unwrap_or(&host.name);
    let flake_target = format!("{}#{}", fleet_root.display(), target_attr);
    args.push("--flake".to_string());
    args.push(flake_target);

    args.push("--extra-files".to_string());
    args.push(host.extra_files_dir.to_string_lossy().to_string());

    if host.ssh_port != 22 {
        args.push("-p".to_string());
        args.push(host.ssh_port.to_string());
        args.push("--post-kexec-ssh-port".to_string());
        args.push(host.ssh_port.to_string());
    }

    if let Some(id) = identity {
        args.push("-i".to_string());
        args.push(id.to_string_lossy().to_string());
    }

    // Hardened rescue environment SSH flags per ADR-008
    args.push("--ssh-option".to_string());
    args.push("UserKnownHostsFile=/dev/null".to_string());
    args.push("--ssh-option".to_string());
    args.push("StrictHostKeyChecking=accept-new".to_string());
    args.push("--ssh-option".to_string());
    args.push("ForwardAgent=no".to_string());

    args.push(format!("root@{}", host.target_ip));

    args
}

pub async fn install_host(
    host: &HostInstall,
    fleet_root: &Path,
    identity: Option<&Path>,
    log_path: &Path,
) -> Result<()> {
    crate::validate::validate_name(Kind::Host, &host.name)?;

    ensure!(
        host.extra_files_dir.exists(),
        NotFoundSnafu {
            kind: Kind::Host,
            name: host.name.clone(),
            path: host.extra_files_dir.clone(),
        }
    );

    ensure!(
        host.extra_files_dir.is_dir(),
        NotADirectorySnafu {
            path: host.extra_files_dir.clone(),
        }
    );

    let abs_identity = crate::ssh::canonicalize_identity(identity)?;
    let args = build_install_args(host, fleet_root, abs_identity.as_deref());

    let mut command = Command::new("nixos-anywhere");
    command.args(&args);
    command.stdin(Stdio::null());
    command.stdout(Stdio::piped());
    command.stderr(Stdio::piped());
    command.kill_on_drop(true);

    let mut child = command.spawn().context(ProcessSpawnSnafu {
        command: "nixos-anywhere",
    })?;

    let stdout = child.stdout.take();
    let stderr = child.stderr.take();

    let log_file = tokio::fs::OpenOptions::new()
        .create(true)
        .write(true)
        .truncate(true)
        .open(log_path)
        .await
        .context(IoSnafu { path: log_path })?;

    let log_writer = Arc::new(tokio::sync::Mutex::new(log_file));

    let host_name = host.name.clone();
    let log_writer_stdout = Arc::clone(&log_writer);
    let stdout_task = tokio::spawn(async move {
        use tokio::io::{AsyncBufReadExt, AsyncWriteExt, BufReader};
        if let Some(stdout) = stdout {
            let mut reader = BufReader::new(stdout).lines();
            while let Ok(Some(line)) = reader.next_line().await {
                let trimmed = line.trim();
                if trimmed.starts_with("### ") {
                    let phase = trimmed.trim_matches('#').trim();
                    crate::ui::info(format!("[{host_name}] {phase}"));
                }
                let mut file = log_writer_stdout.lock().await;
                let _ = file.write_all(line.as_bytes()).await;
                let _ = file.write_all(b"\n").await;
            }
        }
    });

    let log_writer_stderr = Arc::clone(&log_writer);
    let stderr_task = tokio::spawn(async move {
        use tokio::io::{AsyncBufReadExt, AsyncWriteExt, BufReader};
        if let Some(stderr) = stderr {
            let mut reader = BufReader::new(stderr).lines();
            while let Ok(Some(line)) = reader.next_line().await {
                let mut file = log_writer_stderr.lock().await;
                let _ = file.write_all(line.as_bytes()).await;
                let _ = file.write_all(b"\n").await;
            }
        }
    });

    let (exit_status, _, _) = tokio::join!(child.wait(), stdout_task, stderr_task,);

    let exit_status = exit_status.context(ProcessSpawnSnafu {
        command: "nixos-anywhere",
    })?;

    if !exit_status.success() {
        let display_cmd = format!("nixos-anywhere {}", args.join(" "));
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
            target_ip = %host.target_ip,
            log = %log_path.display(),
            "nixos-anywhere installation completed"
        );
    })
}

pub async fn install_fleet(
    hosts: &[HostInstall],
    fleet_root: &Path,
    identity: Option<&Path>,
    parallel_limit: usize,
) -> Vec<InstallResult> {
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
            early_results.push(InstallResult {
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

        let log_path = match crate::process::log::host_log_path("install", &host.name) {
            Ok(p) => p,
            Err(e) => {
                early_results.push(InstallResult {
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
                    install_host(
                        &host_cloned,
                        &fleet_root_cloned,
                        identity_cloned.as_deref(),
                        &log_cloned,
                    )
                    .await
                }
                Err(_) => CommandSignaledSnafu {
                    command: format!("install {}", host_cloned.name),
                }
                .fail(),
            };

            InstallResult {
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
                tracing::warn!("Installation cancelled by operator (Ctrl+C). Terminating running tasks.");
                join_set.abort_all();
                while let Some(join_res) = join_set.join_next().await {
                    match join_res {
                        Ok(res) => results.push(res),
                        Err(join_err) => {
                            if let Some((name, log_path)) = host_lookup.get(&join_err.id()) {
                                results.push(InstallResult {
                                    name: name.clone(),
                                    status: CommandSignaledSnafu {
                                        command: format!("nixos-anywhere on '{name}' (cancelled)"),
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
                            results.push(InstallResult {
                                name: name.clone(),
                                status: CommandSignaledSnafu {
                                    command: format!("nixos-anywhere on '{name}'"),
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
    use tempfile::tempdir;

    #[test]
    fn test_build_install_args_default_port_no_identity() {
        let dir = tempdir().unwrap();
        let host = HostInstall {
            name: "web-01".to_string(),
            flake_attr: None,
            target_ip: "192.168.1.50".to_string(),
            ssh_port: 22,
            extra_files_dir: dir.path().to_path_buf(),
        };

        let args = build_install_args(&host, Path::new("/fleet"), None);

        assert_eq!(
            args,
            vec![
                "--flake",
                "/fleet#web-01",
                "--extra-files",
                dir.path().to_string_lossy().as_ref(),
                "--ssh-option",
                "UserKnownHostsFile=/dev/null",
                "--ssh-option",
                "StrictHostKeyChecking=accept-new",
                "--ssh-option",
                "ForwardAgent=no",
                "root@192.168.1.50"
            ]
        );
    }

    #[test]
    fn test_build_install_args_with_custom_flake_attr() {
        let dir = tempdir().unwrap();
        let host = HostInstall {
            name: "edge".to_string(),
            flake_attr: Some("prod-edge".to_string()),
            target_ip: "10.0.0.1".to_string(),
            ssh_port: 22,
            extra_files_dir: dir.path().to_path_buf(),
        };

        let args = build_install_args(&host, Path::new("/fleet"), None);

        assert_eq!(
            args,
            vec![
                "--flake",
                "/fleet#prod-edge",
                "--extra-files",
                dir.path().to_string_lossy().as_ref(),
                "--ssh-option",
                "UserKnownHostsFile=/dev/null",
                "--ssh-option",
                "StrictHostKeyChecking=accept-new",
                "--ssh-option",
                "ForwardAgent=no",
                "root@10.0.0.1"
            ]
        );
    }

    #[test]
    fn test_build_install_args_custom_port_and_identity() {
        let dir = tempdir().unwrap();
        let host = HostInstall {
            name: "db-01".to_string(),
            flake_attr: None,
            target_ip: "10.0.0.12".to_string(),
            ssh_port: 2222,
            extra_files_dir: dir.path().to_path_buf(),
        };
        let identity = Path::new("/keys/id_ed25519");

        let args = build_install_args(&host, Path::new("/fleet"), Some(identity));

        assert_eq!(
            args,
            vec![
                "--flake",
                "/fleet#db-01",
                "--extra-files",
                dir.path().to_string_lossy().as_ref(),
                "-p",
                "2222",
                "--post-kexec-ssh-port",
                "2222",
                "-i",
                "/keys/id_ed25519",
                "--ssh-option",
                "UserKnownHostsFile=/dev/null",
                "--ssh-option",
                "StrictHostKeyChecking=accept-new",
                "--ssh-option",
                "ForwardAgent=no",
                "root@10.0.0.12"
            ]
        );
    }

    #[tokio::test]
    async fn test_install_host_rejects_missing_extra_files_dir() {
        let host = HostInstall {
            name: "edge-01".to_string(),
            flake_attr: None,
            target_ip: "1.2.3.4".to_string(),
            ssh_port: 22,
            extra_files_dir: PathBuf::from("/nonexistent/extra-files"),
        };
        let log = PathBuf::from("/tmp/test-install.log");
        let result = install_host(&host, Path::new("/fleet"), None, &log).await;
        assert!(result.is_err());
    }

    #[tokio::test]
    async fn test_install_host_rejects_missing_identity() {
        let dir = tempdir().unwrap();
        let host = HostInstall {
            name: "edge-01".to_string(),
            flake_attr: None,
            target_ip: "1.2.3.4".to_string(),
            ssh_port: 22,
            extra_files_dir: dir.path().to_path_buf(),
        };
        let missing_key = dir.path().join("missing_key");
        let log = dir.path().join("install.log");
        let result = install_host(&host, Path::new("/fleet"), Some(&missing_key), &log).await;
        assert!(result.is_err());
    }

    #[tokio::test]
    async fn test_install_fleet_empty_hosts() {
        let results = install_fleet(&[], Path::new("/fleet"), None, 4).await;
        assert!(results.is_empty());
    }

    #[tokio::test]
    async fn test_install_fleet_invalid_hosts_returns_isolated_errors() {
        let hosts = vec![
            HostInstall {
                name: "host-a".to_string(),
                flake_attr: None,
                target_ip: "10.0.0.1".to_string(),
                ssh_port: 22,
                extra_files_dir: PathBuf::from("/nonexistent/extra-files-a"),
            },
            HostInstall {
                name: "host-b".to_string(),
                flake_attr: None,
                target_ip: "10.0.0.2".to_string(),
                ssh_port: 22,
                extra_files_dir: PathBuf::from("/nonexistent/extra-files-b"),
            },
        ];

        let results = install_fleet(&hosts, Path::new("/fleet"), None, 2).await;
        assert_eq!(results.len(), 2);

        let names: std::collections::HashSet<_> = results.iter().map(|r| r.name.as_str()).collect();
        assert!(names.contains("host-a"));
        assert!(names.contains("host-b"));

        for r in results {
            assert!(r.status.is_err());
            assert!(r.log_path.to_string_lossy().contains(&r.name));
        }
    }
}
