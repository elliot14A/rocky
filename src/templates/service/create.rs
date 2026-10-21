use std::path::Path;

use snafu::ensure;
use tap::prelude::*;

use crate::error::{AlreadyExistsSnafu, Kind, Result};
use crate::utils::validate::validate_name;
use crate::{fs, nix};

fn generate_service_nix(name: &str) -> String {
    format!(
        r#"# {name}.nix - service module loaded for hosts whose fleet.nix `services`
# list includes "{name}". A service module is a regular NixOS module.

{{
  config,
  pkgs,
  lib,
  inputs,
  hostName,
  hostConfig,
  environment,
  fleet,
  envVars,
  serviceHost,
  serviceUpstreams,
  replicateService,
  replicateContainers,
  containerRestartTriggers,
  ...
}}:

let
  # Per-environment configuration overrides from environments/${{environment}}/env.nix
  serviceEnv = envVars.{name} or {{ }};

  # Secret configuration (empty by default; uncomment to load agenix secrets)
  # secretNames = [ "{name}.env" ];
  # secretsDir = ../environments + "/${{environment}}/secrets";
  envFiles = [ ]; # map (n: config.age.secrets.${{n}}.path) secretNames;
  secretHashes = [ ]; # map (n: builtins.hashFile "sha256" (secretsDir + "/${{n}}.age")) secretNames;
in
{{
  # ==========================================================================
  # Option A: Native Systemd Service
  # ==========================================================================
  #
  # 1. Single Unit (default):
  # systemd.services.{name} = {{
  #   description = "{name} service";
  #   wantedBy = [ "multi-user.target" ];
  #   serviceConfig = {{
  #     ExecStart = "${{pkgs.example}}/bin/example";
  #     Restart = "on-failure";
  #     EnvironmentFile = envFiles;
  #   }};
  #   restartTriggers = secretHashes;
  #   environment = {{
  #     OTHER_SERVICE_HOST = serviceHost "other-service";
  #   }} // serviceEnv;
  # }};
  #
  # 2. Replicated Units (multi-instance on port offsets):
  # systemd.services = replicateService {{
  #   service = "{name}";
  #   basePort = 8000;
  #   inherit envFiles secretHashes;
  #   mkUnit = {{ index, port, name }}: {{
  #     description = "{name} service (instance ${{toString index}})";
  #     wantedBy = [ "multi-user.target" ];
  #     serviceConfig = {{
  #       ExecStart = "${{pkgs.example}}/bin/example";
  #       Restart = "on-failure";
  #     }};
  #     environment = {{
  #       PORT = toString port;
  #     }} // serviceEnv;
  #   }};
  # }};

  # ==========================================================================
  # Option B: OCI Container via Podman
  # ==========================================================================
  #
  # 1. Single Container (default):
  # virtualisation.oci-containers.containers.{name} = {{
  #   image = "ghcr.io/org/{name}:latest";
  #   autoStart = true;
  #   ports = [ "8080:8080" ];
  #   environmentFiles = envFiles;
  #   environment = {{
  #     OTHER_SERVICE_HOST = serviceHost "other-service";
  #   }} // serviceEnv;
  # }};
  # systemd.services = containerRestartTriggers {{
  #   service = "{name}";
  #   inherit secretHashes;
  # }};
  #
  # 2. Replicated Containers (multi-instance on port offsets):
  # virtualisation.oci-containers.containers = replicateContainers {{
  #   service = "{name}";
  #   basePort = 8080;
  #   containerPort = 8080;
  #   inherit envFiles;
  #   mkContainer = {{ index, port, name }}: {{
  #     image = "ghcr.io/org/{name}:latest";
  #     autoStart = true;
  #     environment = serviceEnv;
  #   }};
  # }};
  # systemd.services = containerRestartTriggers {{
  #   service = "{name}";
  #   inherit secretHashes;
  # }};

  # ==========================================================================
  # Option C: Packaged NixOS Service (e.g. postgresql, caddy, redis)
  # ==========================================================================
  # services.redis.servers.{name}.enable = true;
  #
  # Gateway / Ingress Reverse Proxy Example:
  # Route traffic across all replicated backends on the mesh:
  # services.caddy.virtualHosts."api.example.com".extraConfig = ''
  #   reverse_proxy ${{lib.concatStringsSep " " (serviceUpstreams {{ service = "api"; basePort = 8000; }})}}
  # '';
}}
"#
    )
}

pub async fn create(name: &str, fleet_root: &Path) -> Result<()> {
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

    let services_dir_existed = services_dir.exists();
    fs::ensure_dir(&services_dir)?;

    let mut rollback = fs::Rollback::new();
    if services_dir_existed {
        rollback.remove_file_on_failure(&service_path);
    } else {
        rollback.remove_dir_on_failure(&services_dir);
    }

    generate_service_nix(name).pipe(|content| fs::write_file(&service_path, &content))?;

    nix::fmt([&service_path]).await?;

    rollback.commit();

    Ok(()).tap_ok(|_| {
        tracing::info!(service = %name, "scaffolded service");
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::tempdir;

    #[tokio::test]
    async fn test_create_service_success() {
        let dir = tempdir().unwrap();
        let fleet_root = dir.path();

        create("api", fleet_root).await.unwrap();

        let service_path = fleet_root.join("services").join("api.nix");
        assert!(service_path.is_file());

        let content = std::fs::read_to_string(&service_path).unwrap();
        assert!(content.contains("# api.nix"));
        assert!(content.contains("config,"));
        assert!(content.contains("lib,"));
        assert!(content.contains("fleet,"));
        assert!(content.contains("serviceHost,"));
        assert!(content.contains("systemd.services.api"));
        assert!(content.contains("virtualisation.oci-containers.containers.api"));
    }

    #[tokio::test]
    async fn test_create_service_rejects_duplicate() {
        let dir = tempdir().unwrap();
        let fleet_root = dir.path();

        create("api", fleet_root).await.unwrap();

        let err = create("api", fleet_root).await.unwrap_err();
        match err {
            crate::error::RockyError::AlreadyExists { kind, name, .. } => {
                assert_eq!(kind, Kind::Service);
                assert_eq!(name, "api");
            }
            other => panic!("expected AlreadyExists error, got {other:?}"),
        }
    }

    #[tokio::test]
    async fn test_create_service_rejects_dangling_symlink() {
        let dir = tempdir().unwrap();
        let fleet_root = dir.path();
        let services_dir = fleet_root.join("services");
        std::fs::create_dir_all(&services_dir).unwrap();

        let broken_symlink = services_dir.join("broken.nix");
        std::os::unix::fs::symlink(fleet_root.join("nonexistent.nix"), &broken_symlink).unwrap();

        let err = create("broken", fleet_root).await.unwrap_err();
        match err {
            crate::error::RockyError::AlreadyExists { kind, name, .. } => {
                assert_eq!(kind, Kind::Service);
                assert_eq!(name, "broken");
            }
            other => panic!("expected AlreadyExists error, got {other:?}"),
        }
    }

    #[tokio::test]
    async fn test_create_service_rejects_invalid_name() {
        let dir = tempdir().unwrap();
        let fleet_root = dir.path();

        let err = create("-invalid-name", fleet_root).await.unwrap_err();
        match err {
            crate::error::RockyError::InvalidName { kind, name } => {
                assert_eq!(kind, Kind::Service);
                assert_eq!(name, "-invalid-name");
            }
            other => panic!("expected InvalidName error, got {other:?}"),
        }
    }

    #[tokio::test]
    async fn test_create_service_preserves_existing_services_directory() {
        let dir = tempdir().unwrap();
        let fleet_root = dir.path();
        let services_dir = fleet_root.join("services");
        std::fs::create_dir_all(&services_dir).unwrap();
        let existing_service = services_dir.join("caddy.nix");
        std::fs::write(&existing_service, "{ }: { }").unwrap();

        create("api", fleet_root).await.unwrap();

        assert!(existing_service.is_file());
        assert!(services_dir.join("api.nix").is_file());
    }
}
