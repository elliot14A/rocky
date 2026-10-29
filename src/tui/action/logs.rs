use std::path::Path;

use snafu::{ensure, ResultExt};

use crate::error::{MeshSnafu, ProcessSpawnSnafu, Result};
use crate::process::mesh;
use crate::process::ssh::{
    build_ssh_args, resolve_identity_key, resolve_target_address, ROCKY_USER,
};

pub fn format_journal_cmd(service_unit: Option<&str>) -> String {
    if let Some(unit) = service_unit {
        let clean = unit.trim_end_matches(".service");
        let base = clean.strip_prefix("podman-").unwrap_or(clean);
        format!(
            "journalctl -u '{base}.service' -u '{base}-*.service' -u 'podman-{base}.service' -u 'podman-{base}-*.service' -n 100 --no-pager"
        )
    } else {
        "journalctl -n 100 --no-pager".to_string()
    }
}

pub async fn prepare_logs_args(
    fleet_root: &Path,
    env_name: &str,
    host_name: &str,
    service_unit: Option<&str>,
    identity_arg: Option<&Path>,
) -> Result<Vec<String>> {
    let domain = mesh::resolve_domain(fleet_root, env_name).await?;
    let (is_joined, _) = mesh::check_workstation_tailscale(&domain, env_name).await?;
    ensure!(
        is_joined,
        MeshSnafu {
            reason: format!(
                "Workstation is not connected to fleet mesh '{env_name}'. Join mesh first."
            ),
        }
    );

    let mesh_node = format!("{env_name}-{host_name}");
    let resolved_addr = resolve_target_address(&mesh_node, &domain).await;
    let identity = resolve_identity_key(identity_arg)?;

    let remote_cmd = format_journal_cmd(service_unit);

    Ok(build_ssh_args(
        identity.as_deref(),
        &resolved_addr,
        ROCKY_USER,
        &[remote_cmd],
    ))
}

pub async fn execute(
    fleet_root: &Path,
    env_name: &str,
    host_name: &str,
    service_unit: Option<&str>,
    identity_arg: Option<&Path>,
) -> Result<Vec<String>> {
    let ssh_args =
        prepare_logs_args(fleet_root, env_name, host_name, service_unit, identity_arg).await?;

    let output = tokio::process::Command::new("ssh")
        .args(&ssh_args)
        .kill_on_drop(true)
        .output()
        .await
        .context(ProcessSpawnSnafu {
            command: format!("ssh {}", ssh_args.join(" ")),
        })?;

    let stdout_str = String::from_utf8_lossy(&output.stdout);
    let stderr_str = String::from_utf8_lossy(&output.stderr);

    let mut lines = Vec::new();
    for l in stdout_str.lines() {
        if !l.trim().is_empty() {
            lines.push(l.to_string());
        }
    }
    if !output.status.success() {
        for l in stderr_str.lines() {
            if !l.trim().is_empty() {
                lines.push(l.to_string());
            }
        }
    }
    Ok(lines)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_format_journal_cmd_variants() {
        let cmd_none = format_journal_cmd(None);
        assert_eq!(cmd_none, "journalctl -n 100 --no-pager");

        let cmd_svc = format_journal_cmd(Some("postgrest.service"));
        assert!(cmd_svc.contains("-u 'postgrest.service'"));
        assert!(cmd_svc.contains("-u 'postgrest-*.service'"));
        assert!(cmd_svc.contains("-u 'podman-postgrest.service'"));

        let cmd_podman = format_journal_cmd(Some("podman-gotrue"));
        assert!(cmd_podman.contains("-u 'gotrue.service'"));
        assert!(cmd_podman.contains("-u 'podman-gotrue.service'"));
    }

    #[tokio::test]
    async fn test_logs_unjoined_mesh_fails() {
        let temp = tempfile::tempdir().unwrap();
        let env_dir = temp
            .path()
            .join("environments")
            .join("unjoined-test-env-x9z");
        std::fs::create_dir_all(&env_dir).unwrap();
        std::fs::write(
            env_dir.join("fleet.nix"),
            r#"{ settings.domain = "unjoined-test.internal"; hosts = {}; }"#,
        )
        .unwrap();

        let result =
            prepare_logs_args(temp.path(), "unjoined-test-env-x9z", "app", None, None).await;
        assert!(result.is_err());
        assert!(result.unwrap_err().to_string().contains("fleet mesh"));
    }
}
