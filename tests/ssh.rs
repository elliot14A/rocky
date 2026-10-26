mod common;

use std::fs;

use common::{setup_fleet, TestWorkspace};
use rocky::commands::ssh::{self, SshArgs};
use rocky::process::ssh::{build_ssh_args, parse_target};

#[tokio::test]
async fn test_ssh_rejects_missing_hyphen() {
    let ws = TestWorkspace::new();
    let fleet_path = setup_fleet(&ws);

    let args = SshArgs {
        target: "invalidtarget".to_string(),
        args: vec![],
    };

    let result = ssh::run(args, Some(&fleet_path)).await;
    assert!(result.is_err());
    let err_str = result.unwrap_err().to_string();
    assert!(err_str.contains("expected target in <env>-<host> format"));
}

#[tokio::test]
async fn test_ssh_rejects_nonexistent_environment() {
    let ws = TestWorkspace::new();
    let fleet_path = setup_fleet(&ws);

    let args = SshArgs {
        target: "nonexistent-db".to_string(),
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
        target: "prod-ghost".to_string(),
        args: vec![],
    };

    let result = ssh::run(args, Some(&fleet_path)).await;
    assert!(result.is_err());
    let err_str = result.unwrap_err().to_string();
    assert!(err_str.contains("Host 'ghost' not found"));
}

#[tokio::test]
async fn test_ssh_parse_target_with_hyphenated_environment() {
    let ws = TestWorkspace::new();
    let fleet_path = setup_fleet(&ws);

    let env_dir = fleet_path.join("environments").join("staging-west");
    fs::create_dir_all(&env_dir).unwrap();

    let (env, host) = parse_target(&fleet_path, "staging-west-api").unwrap();
    assert_eq!(env, "staging-west");
    assert_eq!(host, "api");
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
            "ForwardAgent=no",
            "-o",
            "StrictHostKeyChecking=accept-new",
            "rocky@100.64.0.5",
        ]
    );
}
