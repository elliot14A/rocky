use std::path::Path;
use std::time::{Duration, Instant};

use serde::Deserialize;
use tap::prelude::*;
use tokio::time::sleep;

use crate::error::{Kind, Result, TailscaleProbeTimeoutSnafu};
use crate::validate_name;

#[derive(Deserialize)]
struct TailscaleBackendStatus {
    #[serde(rename = "BackendState")]
    backend_state: Option<String>,
}

pub fn build_probe_args(port: u16, identity: Option<&Path>, target: &str) -> Vec<String> {
    let mut args = vec![
        "-F".to_string(),
        "/dev/null".to_string(),
        "-o".to_string(),
        "BatchMode=yes".to_string(),
        "-o".to_string(),
        "StrictHostKeyChecking=accept-new".to_string(),
        "-o".to_string(),
        "UserKnownHostsFile=/dev/null".to_string(),
        "-o".to_string(),
        "ConnectTimeout=3".to_string(),
        "-p".to_string(),
        port.to_string(),
    ];

    if let Some(id) = identity {
        args.push("-i".to_string());
        args.push(id.display().to_string());
        args.push("-o".to_string());
        args.push("IdentitiesOnly=yes".to_string());
    }

    args.push(format!("{}@{target}", crate::process::ssh::ROCKY_USER));
    args.push("tailscale".to_string());
    args.push("status".to_string());
    args.push("--json".to_string());
    args
}

pub fn is_tailscale_online(output: &str) -> bool {
    let trimmed = output.trim();
    if trimmed.is_empty() {
        return false;
    }

    serde_json::from_str::<TailscaleBackendStatus>(trimmed)
        .map(|status| status.backend_state.as_deref() == Some("Running"))
        .unwrap_or(false)
}

pub async fn probe_tailscale_connectivity(
    env: &str,
    host: &str,
    domain: &str,
    target: &str,
    port: u16,
    identity: Option<&Path>,
    timeout_seconds: u64,
) -> Result<()> {
    validate_name(Kind::Environment, env)?;
    validate_name(Kind::Host, host)?;

    let identity_path = crate::ssh::canonicalize_identity(identity)?;
    let timeout = Duration::from_secs(timeout_seconds);
    let start = Instant::now();
    let mut last_stderr = String::new();

    let full_node_name = format!("{env}-{host}");

    while start.elapsed() < timeout {
        let mesh_ip = crate::process::ssh::resolve_target_address(&full_node_name, domain).await;
        let candidate_target = if mesh_ip != full_node_name && !mesh_ip.is_empty() {
            mesh_ip.as_str()
        } else {
            target
        };

        let args = build_probe_args(port, identity_path.as_deref(), candidate_target);
        let mut cmd = tokio::process::Command::new("ssh");
        cmd.args(&args).kill_on_drop(true);

        let command_timeout = Duration::from_secs(4);
        match tokio::time::timeout(command_timeout, cmd.output()).await {
            Ok(Ok(output)) if output.status.success() => {
                let stdout = String::from_utf8_lossy(&output.stdout);
                if is_tailscale_online(&stdout) {
                    return Ok(())
                        .tap_ok(|_| tracing::debug!(env, host, target, port, "tailscale mesh probe succeeded"));
                }
                last_stderr = "Tailscale daemon is running but backend state is not Running".to_string();
            }
            Ok(Ok(output)) => {
                last_stderr = String::from_utf8_lossy(&output.stderr)
                    .trim()
                    .to_string()
                    .pipe(|err| {
                        if err.is_empty() {
                            String::from_utf8_lossy(&output.stdout).trim().to_string()
                        } else {
                            err
                        }
                    });
            }
            Ok(Err(err)) => {
                last_stderr = err.to_string();
            }
            Err(_) => {
                last_stderr = "SSH command timed out during probe attempt".to_string();
            }
        }

        sleep(Duration::from_millis(1000)).await;
    }

    TailscaleProbeTimeoutSnafu {
        host: host.to_string(),
        target: target.to_string(),
        port,
        timeout: timeout_seconds,
        stderr: last_stderr,
    }
    .fail()
    .tap_err(|err| tracing::warn!(%err, env, host, target, "tailscale mesh probe timed out"))
}

#[cfg(test)]
mod tests {
    use std::path::PathBuf;

    use super::*;

    #[test]
    fn test_build_probe_args_without_identity() {
        let args = build_probe_args(22, None, "10.0.0.1");
        assert_eq!(
            args,
            vec![
                "-F",
                "/dev/null",
                "-o",
                "BatchMode=yes",
                "-o",
                "StrictHostKeyChecking=accept-new",
                "-o",
                "UserKnownHostsFile=/dev/null",
                "-o",
                "ConnectTimeout=3",
                "-p",
                "22",
                "rocky@10.0.0.1",
                "tailscale",
                "status",
                "--json"
            ]
        );
    }

    #[test]
    fn test_build_probe_args_with_identity() {
        let id = PathBuf::from("/home/user/.ssh/id_ed25519");
        let args = build_probe_args(2222, Some(&id), "node-01");
        assert_eq!(
            args,
            vec![
                "-F",
                "/dev/null",
                "-o",
                "BatchMode=yes",
                "-o",
                "StrictHostKeyChecking=accept-new",
                "-o",
                "UserKnownHostsFile=/dev/null",
                "-o",
                "ConnectTimeout=3",
                "-p",
                "2222",
                "-i",
                "/home/user/.ssh/id_ed25519",
                "-o",
                "IdentitiesOnly=yes",
                "rocky@node-01",
                "tailscale",
                "status",
                "--json"
            ]
        );
    }

    #[test]
    fn test_is_tailscale_online_checks() {
        assert!(is_tailscale_online(r#"{"BackendState": "Running"}"#));
        assert!(is_tailscale_online(r#"{"BackendState":"Running"}"#));
        assert!(!is_tailscale_online(r#"{"BackendState": "Starting"}"#));
        assert!(!is_tailscale_online(r#"{"BackendState": "NeedsLogin"}"#));
        assert!(!is_tailscale_online(r#"{"BackendState": "Stopped"}"#));
        assert!(!is_tailscale_online("Logged out."));
        assert!(!is_tailscale_online("Tailscale is stopped."));
        assert!(!is_tailscale_online(""));
    }
}
