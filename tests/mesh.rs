mod common;

use std::fs;

use common::{setup_fleet, TestWorkspace};
use rocky::commands::mesh::{self, join::JoinArgs, MeshArgs, MeshCommand};
use rocky::process::mesh::{resolve_authkey, resolve_login_server};

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
async fn test_mesh_resolve_authkey_explicit_arg() {
    let ws = TestWorkspace::new();
    let fleet_path = setup_fleet(&ws);
    let key = resolve_authkey(&fleet_path, "prod", Some("tskey-auth-test-key"), None)
        .await
        .unwrap();
    assert_eq!(key, "tskey-auth-test-key");
}

#[tokio::test]
async fn test_mesh_resolve_authkey_fails_when_secret_missing() {
    let ws = TestWorkspace::new();
    let fleet_path = setup_fleet(&ws);
    let env_dir = fleet_path.join("environments").join("prod");
    fs::create_dir_all(&env_dir).unwrap();

    let err = resolve_authkey(&fleet_path, "prod", None, None).await.unwrap_err();
    assert!(
        err.to_string().contains("Tailscale authkey secret not found"),
        "expected missing secret error, got: {err}"
    );
}

#[tokio::test]
async fn test_mesh_resolve_authkey_from_encrypted_fleet_secret() {
    let ws = TestWorkspace::new();
    let fleet_path = setup_fleet(&ws);
    let env_dir = fleet_path.join("environments").join("prod");
    let secrets_dir = env_dir.join("secrets");
    fs::create_dir_all(&secrets_dir).unwrap();

    let gen = rocky::ssh::generate_host_key("testoperator").await.unwrap();
    let priv_key = gen.extra_files_dir.path().join("etc/ssh/ssh_host_ed25519_key");

    let authkey_value = "hskey-auth-test-roundtrip-secret-token";
    let ciphertext = rocky::crypto::age::encrypt(
        authkey_value.as_bytes(),
        std::slice::from_ref(&gen.public_key),
    )
        .await
        .unwrap();
    fs::write(secrets_dir.join("tailscale-authkey.age"), ciphertext).unwrap();

    let resolved = resolve_authkey(&fleet_path, "prod", None, Some(&priv_key))
        .await
        .unwrap();
    assert_eq!(resolved, authkey_value);
}





#[tokio::test]
async fn test_mesh_join_rejects_missing_environment() {
    let ws = TestWorkspace::new();
    let fleet_path = setup_fleet(&ws);

    let args = MeshArgs {
        command: MeshCommand::Join(JoinArgs {
            env: Some("nonexistent".to_string()),
            env_flag: None,
            authkey: Some("test-key".to_string()),
            identity: None,
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
            env_flag: None,
        }),
    };

    let result = mesh::run(args, Some(&fleet_path)).await;
    assert!(result.is_ok());

    let args_with_flag = MeshArgs {
        command: MeshCommand::Status(rocky::commands::mesh::status::StatusArgs {
            env: None,
            env_flag: Some("prod".to_string()),
        }),
    };

    let result_flag = mesh::run(args_with_flag, Some(&fleet_path)).await;
    assert!(result_flag.is_ok());
}
