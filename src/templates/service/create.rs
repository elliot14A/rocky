use std::path::Path;

use snafu::ensure;
use tap::prelude::*;

use super::kind::ServiceKind;
use crate::error::{AlreadyExistsSnafu, Kind, NotFoundSnafu, Result};
use crate::validate::validate_name;
use crate::{fs, nix};

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ServiceCreationResult {
    ScaffoldedOnly,
    AssignedToHost {
        host: String,
        environment: String,
    },
    MultipleHostsNotice {
        environment: String,
        hosts: Vec<String>,
    },
}

fn generate_service_nix(name: &str, kind: ServiceKind, replicas: u32, port: Option<u16>) -> String {
    let base_port = port.unwrap_or(8080);

    match (kind, replicas > 1) {
        (ServiceKind::Systemd, false) => {
            let port_env = port
                .map(|p| format!(" // {{\n    PORT = \"{p}\";\n  }}"))
                .unwrap_or_default();
            format!(
                r#"# {name}.nix - native systemd service module loaded for hosts whose
# fleet.nix `services` list includes "{name}".

{{
  config,
  pkgs,
  lib,
  environment,
  fleet,
  envVars,
  serviceHost,
  serviceUpstreams,
  replicateService,
  ...
}}:

let
  serviceEnv = envVars.{name} or {{ }};
  secretsDir = ../environments + "/${{environment}}/secrets";
  envFiles = [ ];
  secretHashes = [ ];
in
{{
  systemd.services.{name} = {{
    description = "{name} service";
    wantedBy = [ "multi-user.target" ];
    after = [ "network.target" ];
    serviceConfig = {{
      ExecStart = "${{pkgs.coreutils}}/bin/true";
      Restart = "on-failure";
    }};
    environment = serviceEnv{port_env};
  }};
}}
"#
            )
        }
        (ServiceKind::Systemd, true) => format!(
            r#"# {name}.nix - replicated systemd service module loaded for hosts whose
# fleet.nix `services` list includes "{name}".

{{
  config,
  pkgs,
  lib,
  environment,
  fleet,
  envVars,
  serviceHost,
  serviceUpstreams,
  replicateService,
  ...
}}:

let
  serviceEnv = envVars.{name} or {{ }};
  secretsDir = ../environments + "/${{environment}}/secrets";
  envFiles = [ ];
  secretHashes = [ ];
  basePort = {base_port};
in
{{
  systemd.services = replicateService {{
    service = "{name}";
    inherit basePort envFiles secretHashes;
    mkUnit =
      {{
        index,
        port,
        name,
      }}:
      {{
        description = "{name} service instance ${{toString index}}";
        wantedBy = [ "multi-user.target" ];
        after = [ "network.target" ];
        serviceConfig = {{
          ExecStart = "${{pkgs.coreutils}}/bin/true";
          Restart = "on-failure";
        }};
        environment = serviceEnv // {{
          PORT = toString port;
        }};
      }};
  }};
}}
"#
        ),
        (ServiceKind::Podman, false) => {
            let port_env = port
                .map(|p| format!(" // {{\n    PORT = \"{p}\";\n  }}"))
                .unwrap_or_default();
            format!(
                r#"# {name}.nix - OCI container service module loaded for hosts whose
# fleet.nix `services` list includes "{name}".

{{
  config,
  pkgs,
  lib,
  environment,
  fleet,
  envVars,
  serviceHost,
  serviceUpstreams,
  replicateContainers,
  containerRestartTriggers,
  ...
}}:

let
  serviceEnv = envVars.{name} or {{ }};
  secretsDir = ../environments + "/${{environment}}/secrets";
  envFiles = [ ];
  secretHashes = [ ];
in
{{
  virtualisation.oci-containers.containers.{name} = {{
    image = "alpine:latest";
    autoStart = true;
    environment = serviceEnv{port_env};
    extraOptions = [
      "--network=host"
    ];
  }};

  systemd.services = containerRestartTriggers {{
    service = "{name}";
    inherit secretHashes;
  }};
}}
"#
            )
        }
        (ServiceKind::Podman, true) => format!(
            r#"# {name}.nix - replicated OCI container service module loaded for hosts whose
# fleet.nix `services` list includes "{name}".

{{
  config,
  pkgs,
  lib,
  environment,
  fleet,
  envVars,
  serviceHost,
  serviceUpstreams,
  replicateContainers,
  containerRestartTriggers,
  ...
}}:

let
  serviceEnv = envVars.{name} or {{ }};
  secretsDir = ../environments + "/${{environment}}/secrets";
  envFiles = [ ];
  secretHashes = [ ];
  basePort = {base_port};
  containerPort = {base_port};
in
{{
  virtualisation.oci-containers.containers = replicateContainers {{
    service = "{name}";
    inherit basePort containerPort envFiles;
    mkContainer =
      {{
        index,
        port,
        name,
      }}:
      {{
        image = "alpine:latest";
        autoStart = true;
        environment = serviceEnv // {{
          PORT = toString port;
        }};
      }};
  }};

  systemd.services = containerRestartTriggers {{
    service = "{name}";
    inherit secretHashes;
  }};
}}
"#
        ),
    }
}

pub async fn create(
    name: &str,
    kind: ServiceKind,
    replicas: u32,
    port: Option<u16>,
    env: Option<&str>,
    host: Option<&str>,
    fleet_root: &Path,
) -> Result<ServiceCreationResult> {
    validate_name(Kind::Service, name)?;

    let services_dir = fleet_root.join("services");
    let service_path = services_dir.join(format!("{name}.nix"));

    ensure!(
        service_path.symlink_metadata().is_err(),
        AlreadyExistsSnafu {
            kind: Kind::Service,
            name: name.to_string(),
            path: service_path,
        }
    );

    // Pre-flight validation: check environment and target host before creating any files.
    let mut target_host: Option<(String, std::path::PathBuf, String, String)> = None;
    let mut multiple_hosts: Option<(String, Vec<String>)> = None;

    if host.is_some() || env.is_some() {
        let (env_name, env_dir) = nix::resolve_environment(fleet_root, env)?;
        let fleet_nix = env_dir.join("fleet.nix");
        if fleet_nix.is_file() {
            let content = fs::read_file(&fleet_nix)?;
            let host_map = nix::extract_host_services(&content, &fleet_nix)?;

            if let Some(h) = host {
                validate_name(Kind::Host, h)?;
                ensure!(
                    host_map.contains_key(h),
                    NotFoundSnafu {
                        kind: Kind::Host,
                        name: h.to_string(),
                        path: fleet_nix,
                    }
                );
                target_host = Some((h.to_string(), fleet_nix, env_name, content));
            } else if host_map.len() == 1 {
                let single = host_map.keys().next().cloned().unwrap_or_default();
                target_host = Some((single, fleet_nix, env_name, content));
            } else if !host_map.is_empty() {
                let mut sorted: Vec<_> = host_map.keys().cloned().collect();
                sorted.sort();
                multiple_hosts = Some((env_name, sorted));
            }
        }
    } else if let Ok((env_name, env_dir)) = nix::resolve_environment(fleet_root, None) {
        let fleet_nix = env_dir.join("fleet.nix");
        if fleet_nix.is_file() {
            let content = fs::read_file(&fleet_nix)?;
            let host_map = nix::extract_host_services(&content, &fleet_nix)?;

            if host_map.len() == 1 {
                let single = host_map.keys().next().cloned().unwrap_or_default();
                target_host = Some((single, fleet_nix, env_name, content));
            } else if !host_map.is_empty() {
                let mut sorted: Vec<_> = host_map.keys().cloned().collect();
                sorted.sort();
                multiple_hosts = Some((env_name, sorted));
            }
        }
    }

    let services_dir_existed = services_dir.exists();
    fs::ensure_dir(&services_dir)?;

    let mut rollback = fs::Rollback::new();
    if services_dir_existed {
        rollback.remove_file_on_failure(&service_path);
    } else {
        rollback.remove_dir_on_failure(&services_dir);
    }
    if let Some((_, ref fleet_nix_path, _, ref original_content)) = target_host {
        rollback.restore_file_on_failure(fleet_nix_path, original_content);
    }

    generate_service_nix(name, kind, replicas, port)
        .pipe(|content| fs::write_file(&service_path, &content))?;

    if let Some((ref target_h, ref fleet_nix_path, _, _)) = target_host {
        nix::register_service_to_host(fleet_nix_path, target_h, name, replicas, port).await?;
    }

    nix::fmt([&service_path]).await?;

    rollback.commit();

    tracing::info!(service = %name, %kind, replicas = replicas, "scaffolded service");

    if let Some((target_h, _, env_name, _)) = target_host {
        Ok(ServiceCreationResult::AssignedToHost {
            host: target_h,
            environment: env_name,
        })
    } else if let Some((env_name, hosts)) = multiple_hosts {
        Ok(ServiceCreationResult::MultipleHostsNotice {
            environment: env_name,
            hosts,
        })
    } else {
        Ok(ServiceCreationResult::ScaffoldedOnly)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::tempdir;

    #[tokio::test]
    async fn test_create_service_success() {
        let dir = tempdir().unwrap();
        let service_path = dir.path().join("services").join("my-service.nix");

        let result =
            create("my-service", ServiceKind::Systemd, 1, None, None, None, dir.path()).await;
        assert!(result.is_ok());
        assert!(service_path.is_file());

        let content = std::fs::read_to_string(&service_path).unwrap();
        assert!(content.contains("# my-service.nix"));
        assert!(content.contains("systemd.services.my-service"));
    }

    #[tokio::test]
    async fn test_create_service_podman_replicated() {
        let dir = tempdir().unwrap();
        let service_path = dir.path().join("services").join("worker.nix");

        let result = create(
            "worker",
            ServiceKind::Podman,
            3,
            Some(9000),
            None,
            None,
            dir.path(),
        )
        .await;
        assert!(result.is_ok());
        assert!(service_path.is_file());

        let content = std::fs::read_to_string(&service_path).unwrap();
        assert!(content.contains("replicateContainers"));
        assert!(content.contains("basePort = 9000;"));
    }

    #[tokio::test]
    async fn test_create_service_rejects_duplicate() {
        let dir = tempdir().unwrap();

        create(
            "my-service",
            ServiceKind::Systemd,
            1,
            None,
            None,
            None,
            dir.path(),
        )
        .await
        .unwrap();

        let result =
            create("my-service", ServiceKind::Systemd, 1, None, None, None, dir.path()).await;
        assert!(result.is_err());
        match result.unwrap_err() {
            crate::error::RockyError::AlreadyExists { kind, name, .. } => {
                assert_eq!(kind, Kind::Service);
                assert_eq!(name, "my-service");
            }
            other => panic!("expected AlreadyExists, got {:?}", other),
        }
    }

    #[tokio::test]
    async fn test_create_service_rejects_dangling_symlink() {
        let dir = tempdir().unwrap();
        let services_dir = dir.path().join("services");
        std::fs::create_dir_all(&services_dir).unwrap();

        let service_path = services_dir.join("broken.nix");
        std::os::unix::fs::symlink(dir.path().join("nonexistent"), &service_path).unwrap();

        let result = create("broken", ServiceKind::Systemd, 1, None, None, None, dir.path()).await;
        assert!(result.is_err());
        match result.unwrap_err() {
            crate::error::RockyError::AlreadyExists { kind, name, .. } => {
                assert_eq!(kind, Kind::Service);
                assert_eq!(name, "broken");
            }
            other => panic!("expected AlreadyExists, got {:?}", other),
        }
    }

    #[tokio::test]
    async fn test_create_service_rejects_invalid_name() {
        let dir = tempdir().unwrap();

        let result =
            create("-invalid", ServiceKind::Systemd, 1, None, None, None, dir.path()).await;
        assert!(result.is_err());
        match result.unwrap_err() {
            crate::error::RockyError::InvalidName { kind, name } => {
                assert_eq!(kind, Kind::Service);
                assert_eq!(name, "-invalid");
            }
            other => panic!("expected InvalidName, got {:?}", other),
        }
    }

    #[tokio::test]
    async fn test_create_service_preserves_existing_services_directory() {
        let dir = tempdir().unwrap();
        let services_dir = dir.path().join("services");
        std::fs::create_dir_all(&services_dir).unwrap();

        let existing_file = services_dir.join("existing.nix");
        std::fs::write(&existing_file, "# existing").unwrap();

        create(
            "new-service",
            ServiceKind::Systemd,
            1,
            None,
            None,
            None,
            dir.path(),
        )
        .await
        .unwrap();

        assert!(services_dir.is_dir());
        assert!(existing_file.is_file());
        assert!(services_dir.join("new-service.nix").is_file());
    }
}
