use std::path::Path;

use crate::crypto::secret::create::create_secret;
use crate::error::Result;
use crate::nix;
use crate::templates::env::create as create_environment;
use crate::templates::host::create as create_host;
use crate::templates::service::create as create_service;
use crate::templates::service::ServiceKind;

pub async fn create_host_entity(
    fleet_root: &Path,
    name: &str,
    env_name: &str,
    provider: Option<&str>,
) -> Result<String> {
    let resolved_provider = match provider {
        Some(p) if !p.trim().is_empty() => p.to_string(),
        _ => nix::eval_default_provider(fleet_root)
            .await
            .unwrap_or_else(|_| "default".to_string()),
    };

    create_host(name, Some(env_name), &resolved_provider, fleet_root).await?;
    Ok(format!(
        "Created host '{name}' ({resolved_provider}) in '{env_name}'"
    ))
}

pub async fn create_service_entity(
    fleet_root: &Path,
    name: &str,
    kind: ServiceKind,
    target_host: Option<&str>,
    port: Option<u16>,
    replicas: u32,
    env_name: Option<&str>,
) -> Result<String> {
    create_service(
        name,
        kind,
        replicas,
        port,
        env_name,
        target_host,
        fleet_root,
    )
    .await?;

    if let Some(host) = target_host {
        Ok(format!("Created service '{name}' on host '{host}'"))
    } else {
        Ok(format!("Created service '{name}'"))
    }
}

pub async fn create_secret_entity(
    fleet_root: &Path,
    name: &str,
    env_name: &str,
    all_hosts: bool,
    identity: Option<&Path>,
) -> Result<String> {
    let env_dir = fleet_root.join("environments").join(env_name);
    create_secret(&env_dir, name, "", &[], all_hosts).await?;

    let _ = crate::crypto::secret::edit::edit_secret(&env_dir, name, identity).await;

    Ok(format!("Created secret '{name}' in '{env_name}'"))
}

pub async fn create_environment_entity(fleet_root: &Path, name: &str) -> Result<String> {
    create_environment(name, fleet_root).await?;
    Ok(format!("Created environment '{name}'"))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn test_create_environment_validation() {
        let temp = tempfile::tempdir().unwrap();
        let res = create_environment_entity(temp.path(), "invalid/name").await;
        assert!(res.is_err());
    }

    #[tokio::test]
    async fn test_create_host_validation() {
        let temp = tempfile::tempdir().unwrap();
        let res = create_host_entity(temp.path(), "invalid/host", "prod", None).await;
        assert!(res.is_err());
    }

    #[tokio::test]
    async fn test_create_service_validation() {
        let temp = tempfile::tempdir().unwrap();
        let res = create_service_entity(
            temp.path(),
            "invalid/svc",
            crate::templates::service::ServiceKind::Systemd,
            None,
            None,
            1,
            None,
        )
        .await;
        assert!(res.is_err());
    }
}
