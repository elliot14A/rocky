mod common;

use std::fs;

use common::{setup_fleet, TestWorkspace};
use rocky::commands::new::env::{self, EnvArgs};
use rocky::commands::new::host::{self, HostArgs};
use rocky::commands::new::service::{self, ServiceArgs, ServiceKind};
use rocky::error::{Kind, RockyError};

#[tokio::test]
async fn test_new_service_systemd_default() {
    let ws = TestWorkspace::new();
    let fleet_path = setup_fleet(&ws);
    common::setup_env_with_hosts(
        &fleet_path,
        "prod",
        "    \"web-01\" = { services = [ ]; };",
        None,
    );

    let args = ServiceArgs {
        name: "api".to_string(),
        host: None,
        env: None,
        kind: ServiceKind::Systemd,
        replicas: 1,
        port: None,
    };

    let res = service::run(args, Some(&fleet_path)).await;
    assert!(res.is_ok(), "service::run failed: {:?}", res);

    let service_path = fleet_path.join("services").join("api.nix");
    assert!(service_path.is_file());

    let content = fs::read_to_string(&service_path).unwrap();
    assert!(content.contains("# api.nix"));
    assert!(content.contains("config,"));
    assert!(content.contains("lib,"));
    assert!(content.contains("pkgs,"));
    assert!(content.contains("fleet,"));
    assert!(content.contains("serviceHost,"));
    assert!(content.contains("systemd.services.api"));
}

#[tokio::test]
async fn test_new_service_podman_replicated() {
    let ws = TestWorkspace::new();
    let fleet_path = setup_fleet(&ws);
    common::setup_env_with_hosts(
        &fleet_path,
        "prod",
        "    \"web-01\" = { services = [ ]; };",
        None,
    );

    let args = ServiceArgs {
        name: "worker".to_string(),
        host: None,
        env: None,
        kind: ServiceKind::Podman,
        replicas: 2,
        port: Some(9999),
    };

    let res = service::run(args, Some(&fleet_path)).await;
    assert!(res.is_ok(), "service::run failed: {:?}", res);

    let service_path = fleet_path.join("services").join("worker.nix");
    assert!(service_path.is_file());

    let content = fs::read_to_string(&service_path).unwrap();
    assert!(content.contains("replicateContainers"));
    assert!(content.contains("basePort = 9999;"));
    assert!(content.contains("containerPort = 9999;"));
    assert!(content.contains("containerRestartTriggers"));
}

#[tokio::test]
async fn test_new_service_assigns_to_explicit_host_in_fleet() {
    let ws = TestWorkspace::new();
    let fleet_path = setup_fleet(&ws);
    common::setup_env_with_hosts(
        &fleet_path,
        "prod",
        "    \"web-01\" = { services = [ ]; };",
        None,
    );

    // fleet.nix has host "web-01" from setup_fleet
    let args = ServiceArgs {
        name: "postgrest".to_string(),
        host: Some("web-01".to_string()),
        env: Some("prod".to_string()),
        kind: ServiceKind::Systemd,
        replicas: 3,
        port: Some(3000),
    };

    let res = service::run(args, Some(&fleet_path)).await;
    assert!(res.is_ok(), "service::run failed: {:?}", res);

    // Verify service file was populated with basePort
    let service_path = fleet_path.join("services").join("postgrest.nix");
    assert!(service_path.is_file());
    let svc_content = fs::read_to_string(&service_path).unwrap();
    assert!(svc_content.contains("basePort = 3000;"));

    // Verify fleet.nix was automatically populated with the service assignment!
    let fleet_nix_path = fleet_path
        .join("environments")
        .join("prod")
        .join("fleet.nix");
    let fleet_content = fs::read_to_string(&fleet_nix_path).unwrap();
    assert!(fleet_content.contains("postgrest"));
    assert!(fleet_content.contains("replicas = 3;"));
    assert!(fleet_content.contains("basePort = 3000;"));
}

#[tokio::test]
async fn test_new_service_rejects_non_fleet() {
    let ws = TestWorkspace::new();
    let empty_dir = ws.path.join("non-fleet");
    fs::create_dir_all(&empty_dir).unwrap();

    let args = ServiceArgs {
        name: "api".to_string(),
        host: None,
        env: None,
        kind: ServiceKind::Systemd,
        replicas: 1,
        port: None,
    };

    let res = service::run(args, Some(&empty_dir)).await;
    assert!(res.is_err());
    match res.unwrap_err() {
        RockyError::NotFound { kind, .. } => {
            assert_eq!(kind, Kind::Fleet);
        }
        other => panic!("expected NotFound(Kind::Fleet), got {other:?}"),
    }
}

#[tokio::test]
async fn test_new_service_rejects_invalid_name() {
    let ws = TestWorkspace::new();
    let fleet_path = setup_fleet(&ws);
    common::setup_env_with_hosts(
        &fleet_path,
        "prod",
        "    \"web-01\" = { services = [ ]; };",
        None,
    );

    let invalid_names = [
        "-invalid",
        "../evil",
        "path/to/thing",
        "name with spaces",
        "service@name",
    ];
    for name in invalid_names {
        let args = ServiceArgs {
            name: name.to_string(),
            host: None,
            env: None,
            kind: ServiceKind::Systemd,
            replicas: 1,
            port: None,
        };

        let res = service::run(args, Some(&fleet_path)).await;
        assert!(res.is_err(), "expected error for name '{name}'");
        match res.unwrap_err() {
            RockyError::InvalidName {
                kind,
                name: err_name,
            } => {
                assert_eq!(kind, Kind::Service);
                assert_eq!(err_name, name);
            }
            other => panic!("expected InvalidName(Kind::Service) for '{name}', got {other:?}"),
        }
    }
}

#[tokio::test]
async fn test_new_service_rejects_already_existing_service() {
    let ws = TestWorkspace::new();
    let fleet_path = setup_fleet(&ws);
    common::setup_env_with_hosts(
        &fleet_path,
        "prod",
        "    \"web-01\" = { services = [ ]; };",
        None,
    );

    let args = ServiceArgs {
        name: "api".to_string(),
        host: None,
        env: None,
        kind: ServiceKind::Systemd,
        replicas: 1,
        port: None,
    };

    service::run(args.clone(), Some(&fleet_path)).await.unwrap();

    let res = service::run(args, Some(&fleet_path)).await;
    assert!(res.is_err());
    match res.unwrap_err() {
        RockyError::AlreadyExists { kind, name, path } => {
            assert_eq!(kind, Kind::Service);
            assert_eq!(name, "api");
            assert_eq!(path, fleet_path.join("services").join("api.nix"));
        }
        other => panic!("expected AlreadyExists(Kind::Service), got {other:?}"),
    }
}

#[tokio::test]
async fn test_new_service_rejects_nonexistent_host_preflight_no_orphan() {
    let ws = TestWorkspace::new();
    let fleet_path = setup_fleet(&ws);
    common::setup_env_with_hosts(
        &fleet_path,
        "prod",
        "    \"web-01\" = { services = [ ]; };",
        None,
    );

    let args = ServiceArgs {
        name: "payment".to_string(),
        host: Some("ghost-node".to_string()),
        env: Some("prod".to_string()),
        kind: ServiceKind::Systemd,
        replicas: 1,
        port: None,
    };

    let res = service::run(args, Some(&fleet_path)).await;
    assert!(res.is_err());
    match res.unwrap_err() {
        RockyError::NotFound { kind, name, .. } => {
            assert_eq!(kind, Kind::Host);
            assert_eq!(name, "ghost-node");
        }
        other => panic!("expected NotFound(Kind::Host), got {other:?}"),
    }

    // Verify transactional safety: no orphaned service file was created!
    assert!(!fleet_path.join("services").join("payment.nix").exists());
}

#[tokio::test]
async fn test_new_service_evaluates_in_nix_fleet() {
    let ws = TestWorkspace::new();
    ws.init("base", false).await.unwrap();
    let fleet_path = ws.path.join("rocky");

    // 1. Scaffold environment
    env::run(
        EnvArgs {
            name: "prod".to_string(),
        },
        Some(&fleet_path),
    )
    .await
    .unwrap();

    // 2. Scaffold host
    host::run(
        HostArgs {
            name: "node-01".to_string(),
            env: Some("prod".to_string()),
            provider: Some("hetzner".to_string()),
            partition: None,
        },
        Some(&fleet_path),
    )
    .await
    .unwrap();

    // 3. Scaffold service with host assignment directly!
    service::run(
        ServiceArgs {
            name: "worker".to_string(),
            host: Some("node-01".to_string()),
            env: Some("prod".to_string()),
            kind: ServiceKind::Systemd,
            replicas: 1,
            port: None,
        },
        Some(&fleet_path),
    )
    .await
    .unwrap();

    let service_file = fleet_path.join("services").join("worker.nix");
    assert!(service_file.is_file());

    // 4. Verify fleet.nix automatically has the service populated!
    let fleet_nix_path = fleet_path
        .join("environments")
        .join("prod")
        .join("fleet.nix");
    let fleet_nix = fs::read_to_string(&fleet_nix_path).unwrap();
    assert!(fleet_nix.contains("worker"));

    // 5. Evaluate flake with nix flake check
    let output = std::process::Command::new("nix")
        .args(["flake", "check", "--no-build", "path:."])
        .current_dir(&fleet_path)
        .output()
        .expect("failed to execute nix flake check");

    let stderr = String::from_utf8_lossy(&output.stderr);
    assert!(
        output.status.success(),
        "nix flake check failed for fleet with unmodified new service:\n{stderr}"
    );
}
