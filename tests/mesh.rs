mod common;

use std::fs;

use common::{setup_fleet, TestWorkspace};
use rocky::commands::mesh::{self, join::JoinArgs, MeshArgs, MeshCommand};
use rocky::process::mesh::{resolve_login_server, resolve_or_create_authkey};

#[tokio::test]
async fn test_mesh_resolve_login_server_from_fleet_nix() {
    let ws = TestWorkspace::new();
    let fleet_path = setup_fleet(&ws);

    let env_dir = fleet_path.join("environments").join("prod");
    fs::create_dir_all(&env_dir).unwrap();

    let fleet_nix = r#"{
  settings = {
    tailscaleLoginServer = "https://headscale.internal.company.com";
  };
  hosts = { };
}
"#;
    fs::write(env_dir.join("fleet.nix"), fleet_nix).unwrap();

    let server = resolve_login_server(&fleet_path, "prod").await.unwrap();
    assert_eq!(server, "https://headscale.internal.company.com");
}

#[tokio::test]
async fn test_mesh_resolve_login_server_translates_qemu_loopback() {
    let ws = TestWorkspace::new();
    let fleet_path = setup_fleet(&ws);

    let env_dir = fleet_path.join("environments").join("prod");
    fs::create_dir_all(&env_dir).unwrap();

    let fleet_nix = r#"{
  settings = {
    tailscaleLoginServer = "http://10.0.2.2:8080";
  };
  hosts = { };
}
"#;
    fs::write(env_dir.join("fleet.nix"), fleet_nix).unwrap();

    let server = resolve_login_server(&fleet_path, "prod").await.unwrap();
    assert_eq!(server, "http://127.0.0.1:8080");
}

#[tokio::test]
async fn test_mesh_resolve_login_server_defaults_to_tailscale_saas() {
    let ws = TestWorkspace::new();
    let fleet_path = setup_fleet(&ws);

    let env_dir = fleet_path.join("environments").join("staging");
    fs::create_dir_all(&env_dir).unwrap();

    let fleet_nix = r#"{
  hosts = { };
}
"#;
    fs::write(env_dir.join("fleet.nix"), fleet_nix).unwrap();

    let server = resolve_login_server(&fleet_path, "staging").await.unwrap();
    assert_eq!(server, "https://controlplane.tailscale.com");
}

#[tokio::test]
async fn test_mesh_resolve_or_create_authkey_explicit_arg() {
    let key = resolve_or_create_authkey("https://controlplane.tailscale.com", Some("tskey-auth-test-key"))
        .await
        .unwrap();
    assert_eq!(key, "tskey-auth-test-key");
}

static ENV_LOCK: tokio::sync::Mutex<()> = tokio::sync::Mutex::const_new(());

#[tokio::test]
async fn test_mesh_resolve_or_create_authkey_env_var() {
    let _guard = ENV_LOCK.lock().await;
    std::env::set_var("ROCKY_AUTHKEY", "tskey-auth-from-env");
    let key = resolve_or_create_authkey("https://controlplane.tailscale.com", None)
        .await
        .unwrap();
    assert_eq!(key, "tskey-auth-from-env");
    std::env::remove_var("ROCKY_AUTHKEY");
}

#[tokio::test]
async fn test_mesh_join_rejects_missing_environment() {
    let ws = TestWorkspace::new();
    let fleet_path = setup_fleet(&ws);

    let args = MeshArgs {
        command: MeshCommand::Join(JoinArgs {
            env: Some("nonexistent".to_string()),
            authkey: Some("test-key".to_string()),
            login_server: None,
            force_reauth: false,
        }),
    };

    let result = mesh::run(args, Some(&fleet_path)).await;
    assert!(result.is_err());
}

#[tokio::test]
async fn test_mesh_status_returns_declared_fleet_hosts() {
    let ws = TestWorkspace::new();
    let fleet_path = setup_fleet(&ws);

    let env_dir = fleet_path.join("environments").join("prod");
    fs::create_dir_all(&env_dir).unwrap();

    let fleet_nix = r#"{
  settings = {
    tailscaleLoginServer = "https://controlplane.tailscale.com";
    domain = "prod.rocky.internal";
  };
  hosts = {
    "db" = { };
    "edge" = { };
  };
}
"#;
    fs::write(env_dir.join("fleet.nix"), fleet_nix).unwrap();

    let status = rocky::process::mesh::get_fleet_mesh_status(&fleet_path, "prod")
        .await
        .unwrap();

    assert_eq!(status.environment, "prod");
    assert_eq!(status.domain, "prod.rocky.internal");
    assert_eq!(status.hosts.len(), 2);
    assert_eq!(status.hosts[0].name, "prod-db");
    assert_eq!(status.hosts[0].status, "Not registered");
    assert_eq!(status.hosts[0].dns_name, "prod-db");
    assert_eq!(status.hosts[1].name, "prod-edge");
    assert_eq!(status.hosts[1].status, "Not registered");
    assert_eq!(status.hosts[1].dns_name, "prod-edge");
}

#[tokio::test]
async fn test_mesh_status_command_executes_cleanly() {
    let ws = TestWorkspace::new();
    let fleet_path = setup_fleet(&ws);

    let env_dir = fleet_path.join("environments").join("prod");
    fs::create_dir_all(&env_dir).unwrap();

    let fleet_nix = r#"{
  settings = {
    tailscaleLoginServer = "https://controlplane.tailscale.com";
  };
  hosts = {
    "web" = { };
  };
}
"#;
    fs::write(env_dir.join("fleet.nix"), fleet_nix).unwrap();

    let args = MeshArgs {
        command: MeshCommand::Status(rocky::commands::mesh::status::StatusArgs {
            env: Some("prod".to_string()),
        }),
    };

    let result = mesh::run(args, Some(&fleet_path)).await;
    assert!(result.is_ok());
}
