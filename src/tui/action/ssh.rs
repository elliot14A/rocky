use std::path::Path;
use std::process::Stdio;

use snafu::{ensure, ResultExt};

use crate::error::{MeshSnafu, ProcessSpawnSnafu, Result};
use crate::process::mesh;
use crate::process::ssh::{
    build_ssh_args, resolve_identity_key, resolve_target_address, ROCKY_USER,
};

pub async fn prepare_ssh_args(
    fleet_root: &Path,
    env_name: &str,
    host_name: &str,
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
    Ok(build_ssh_args(
        identity.as_deref(),
        &resolved_addr,
        ROCKY_USER,
        &[],
    ))
}

pub async fn execute(
    fleet_root: &Path,
    env_name: &str,
    host_name: &str,
    identity_arg: Option<&Path>,
) -> Result<()> {
    let ssh_args = prepare_ssh_args(fleet_root, env_name, host_name, identity_arg).await?;

    let status = tokio::process::Command::new("ssh")
        .args(&ssh_args)
        .stdin(Stdio::inherit())
        .stdout(Stdio::inherit())
        .stderr(Stdio::inherit())
        .kill_on_drop(true)
        .status()
        .await
        .context(ProcessSpawnSnafu {
            command: format!("ssh {}", ssh_args.join(" ")),
        })?;

    let _ = status;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn test_ssh_unjoined_mesh_fails() {
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

        let result = prepare_ssh_args(temp.path(), "unjoined-test-env-x9z", "app", None).await;
        assert!(result.is_err());
        assert!(result.unwrap_err().to_string().contains("fleet mesh"));
    }
}
