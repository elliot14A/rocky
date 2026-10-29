mod common;

use std::fs;

use common::{setup_fleet, TestWorkspace};
use rocky::commands::ssh::{self, SshArgs};
use rocky::process::ssh::build_ssh_args;

#[tokio::test]
async fn test_ssh_rejects_invalid_environment_name() {
    let ws = TestWorkspace::new();
    let fleet_path = setup_fleet(&ws);

    let args = SshArgs {
        env: "../../invalid".to_string(),
        target: "db".to_string(),
        identity: None,
        args: vec![],
    };

    let result = ssh::run(args, Some(&fleet_path)).await;
    assert!(result.is_err());
    let err_str = result.unwrap_err().to_string();
    assert!(err_str.contains("Invalid Environment '../../invalid'"));
}

#[tokio::test]
async fn test_ssh_rejects_invalid_target_name() {
    let ws = TestWorkspace::new();
    let fleet_path = setup_fleet(&ws);

    let args = SshArgs {
        env: "prod".to_string(),
        target: "../invalid".to_string(),
        identity: None,
        args: vec![],
    };

    let result = ssh::run(args, Some(&fleet_path)).await;
    assert!(result.is_err());
    let err_str = result.unwrap_err().to_string();
    assert!(err_str.contains("Invalid Host '../invalid'"));
}

#[tokio::test]
async fn test_ssh_rejects_ambiguous_service_on_multiple_hosts() {
    let ws = TestWorkspace::new();
    let fleet_path = setup_fleet(&ws);

    let env_dir = fleet_path.join("environments").join("prod");
    fs::create_dir_all(&env_dir).unwrap();

    let fleet_nix = r#"{
  settings = { };
  hosts = {
    "node-01" = { services = [ "api" ]; };
    "node-02" = { services = [ "api" ]; };
  };
}
"#;
    fs::write(env_dir.join("fleet.nix"), fleet_nix).unwrap();

    let args = SshArgs {
        env: "prod".to_string(),
        target: "api".to_string(),
        identity: None,
        args: vec![],
    };

    let result = ssh::run(args, Some(&fleet_path)).await;
    assert!(result.is_err());
    let err_str = result.unwrap_err().to_string();
    assert!(err_str.contains(
        "service 'api' runs on multiple hosts (node-01, node-02); specify the host directly"
    ));
}

#[tokio::test]
async fn test_ssh_rejects_nonexistent_environment() {
    let ws = TestWorkspace::new();
    let fleet_path = setup_fleet(&ws);

    let args = SshArgs {
        env: "nonexistent".to_string(),
        target: "db".to_string(),
        identity: None,
        args: vec![],
    };

    let result = ssh::run(args, Some(&fleet_path)).await;
    assert!(result.is_err());
    let err_str = result.unwrap_err().to_string();
    assert!(err_str.contains("Environment 'nonexistent' not found"));
}

#[tokio::test]
async fn test_ssh_rejects_nonexistent_host_in_environment() {
    let ws = TestWorkspace::new();
    let fleet_path = setup_fleet(&ws);

    let env_dir = fleet_path.join("environments").join("prod");
    fs::create_dir_all(&env_dir).unwrap();

    let fleet_nix = r#"{
  settings = { };
  hosts = {
    "db" = { };
  };
}
"#;
    fs::write(env_dir.join("fleet.nix"), fleet_nix).unwrap();

    let args = SshArgs {
        env: "prod".to_string(),
        target: "ghost".to_string(),
        identity: None,
        args: vec![],
    };

    let result = ssh::run(args, Some(&fleet_path)).await;
    assert!(result.is_err());
    let err_str = result.unwrap_err().to_string();
    assert!(err_str.contains("Host 'ghost' not found"));
}

#[tokio::test]
async fn test_ssh_resolves_service_to_host_or_fails_mesh() {
    let ws = TestWorkspace::new();
    let fleet_path = setup_fleet(&ws);

    let env_dir = fleet_path.join("environments").join("unjoined");
    fs::create_dir_all(&env_dir).unwrap();

    let fleet_nix = r#"{
  settings = { };
  hosts = {
    "edge" = {
      services = [ "caddy" ];
    };
  };
}
"#;
    fs::write(env_dir.join("fleet.nix"), fleet_nix).unwrap();

    let args = SshArgs {
        env: "unjoined".to_string(),
        target: "caddy".to_string(),
        identity: None,
        args: vec![],
    };

    let result = ssh::run(args, Some(&fleet_path)).await;
    // Passes host resolution for caddy -> edge and fails at mesh check for 'unjoined'
    assert!(result.is_err());
    let err_str = result.unwrap_err().to_string();
    assert!(err_str.contains("Workstation is not connected to fleet mesh 'unjoined'"));
}

#[test]
fn test_ssh_build_args_formatting() {
    let extra = vec!["-p".to_string(), "2222".to_string(), "uptime".to_string()];
    let args = build_ssh_args(None, "100.64.0.2", "rocky", &extra);

    assert_eq!(
        args,
        vec![
            "-o",
            "ForwardAgent=no",
            "-o",
            "StrictHostKeyChecking=accept-new",
            "rocky@100.64.0.2",
            "-p",
            "2222",
            "uptime"
        ]
    );
}

#[test]
fn test_ssh_build_args_with_identity_key() {
    let identity = std::path::Path::new("/home/user/.ssh/agenix_ed25519");
    let args = build_ssh_args(Some(identity), "100.64.0.5", "rocky", &[]);

    assert_eq!(
        args,
        vec![
            "-i",
            "/home/user/.ssh/agenix_ed25519",
            "-o",
            "IdentitiesOnly=yes",
            "-o",
            "ForwardAgent=no",
            "-o",
            "StrictHostKeyChecking=accept-new",
            "rocky@100.64.0.5",
        ]
    );
}
