mod common;

use std::fs;
use std::path::PathBuf;

use rocky::commands::new::env::{self, EnvArgs};
use rocky::commands::new::host::{self, HostArgs};
use rocky::error::{Kind, RockyError};

fn setup_fleet(ws: &common::TestWorkspace) -> PathBuf {
    let fleet_path = ws.path.join("rocky");
    fs::create_dir_all(&fleet_path).unwrap();
    fs::write(
        fleet_path.join("flake.nix"),
        r#"{
  description = "rocky test fleet";
  outputs = inputs: {
    rockyFleet = true;
    config = {
      defaultProvider = "hetzner";
    };
  };
}
"#,
    )
    .unwrap();
    fleet_path
}

#[tokio::test]
async fn test_new_env_success() {
    let ws = common::TestWorkspace::new();
    let fleet_path = setup_fleet(&ws);

    let args = EnvArgs {
        name: "staging".to_string(),
    };

    let res = env::run(args, Some(&fleet_path)).await;
    assert!(res.is_ok(), "env::run failed: {:?}", res);

    let env_dir = fleet_path.join("environments").join("staging");
    assert!(env_dir.is_dir());

    let fleet_nix_path = env_dir.join("fleet.nix");
    assert!(fleet_nix_path.is_file());
    let fleet_nix = fs::read_to_string(&fleet_nix_path).unwrap();
    assert!(fleet_nix.contains("staging.rocky.internal"));
    assert!(fleet_nix.contains("hosts = { };"));

    let env_nix_path = env_dir.join("env.nix");
    assert!(env_nix_path.is_file());
    let env_nix = fs::read_to_string(&env_nix_path).unwrap();
    assert!(env_nix.contains("caddy = { };"));

    let keys_path = env_dir.join("keys.nix");
    assert!(keys_path.is_file());

    let secrets_dir = env_dir.join("secrets");
    assert!(secrets_dir.join("secrets.nix").is_file());
    let secrets_nix = fs::read_to_string(secrets_dir.join("secrets.nix")).unwrap();
    assert!(secrets_nix.contains("hostKeys = import ../keys.nix;"));
    assert!(secrets_nix.contains("recipientsFor"));

    assert!(env_dir.join("hosts").is_dir());
}

#[tokio::test]
async fn test_new_env_rejects_already_existing_env() {
    let ws = common::TestWorkspace::new();
    let fleet_path = setup_fleet(&ws);

    let args1 = EnvArgs {
        name: "prod".to_string(),
    };
    let res1 = env::run(args1, Some(&fleet_path)).await;
    assert!(res1.is_ok());

    let args2 = EnvArgs {
        name: "prod".to_string(),
    };
    let res2 = env::run(args2, Some(&fleet_path)).await;
    match res2 {
        Err(RockyError::AlreadyExists { kind, name, .. }) => {
            assert_eq!(kind, Kind::Environment);
            assert_eq!(name, "prod");
        }
        other => panic!("expected AlreadyExists Environment, got {:?}", other),
    }
}

#[tokio::test]
async fn test_new_env_rejects_invalid_name() {
    let ws = common::TestWorkspace::new();
    let fleet_path = setup_fleet(&ws);

    let args = EnvArgs {
        name: "../evil".to_string(),
    };
    let res = env::run(args, Some(&fleet_path)).await;
    match res {
        Err(RockyError::InvalidName { kind, name }) => {
            assert_eq!(kind, Kind::Environment);
            assert_eq!(name, "../evil");
        }
        other => panic!("expected InvalidName Environment, got {:?}", other),
    }
}

#[tokio::test]
async fn test_new_env_rejects_non_fleet() {
    let ws = common::TestWorkspace::new();
    let empty_path = ws.path.join("not-a-fleet");
    fs::create_dir_all(&empty_path).unwrap();

    let args = EnvArgs {
        name: "prod".to_string(),
    };
    let res = env::run(args, Some(&empty_path)).await;
    match res {
        Err(RockyError::NotFound { kind, .. }) => {
            assert_eq!(kind, Kind::Fleet);
        }
        other => panic!("expected NotFound Fleet, got {:?}", other),
    }
}

#[tokio::test]
async fn test_new_env_followed_by_new_host_flow() {
    let ws = common::TestWorkspace::new();
    let fleet_path = setup_fleet(&ws);

    // 1. Scaffold environment
    let env_res = env::run(
        EnvArgs {
            name: "qa".to_string(),
        },
        Some(&fleet_path),
    )
    .await;
    assert!(env_res.is_ok());

    // 2. Scaffold host inside the new environment
    let host_res = host::run(
        HostArgs {
            name: "srv-01".to_string(),
            env: Some("qa".to_string()),
            provider: Some("hetzner".to_string()),
            partition: None,
        },
        Some(&fleet_path),
    )
    .await;
    assert!(host_res.is_ok(), "host::run failed: {:?}", host_res);

    let host_dir = fleet_path
        .join("environments")
        .join("qa")
        .join("hosts")
        .join("srv-01");
    assert!(host_dir.join("default.nix").is_file());
    assert!(host_dir.join("disko.nix").is_file());
    assert!(host_dir.join("hardware.nix").is_file());

    let default_nix = fs::read_to_string(host_dir.join("default.nix")).unwrap();
    assert!(default_nix.contains("qa-srv-01"));

    let fleet_nix =
        fs::read_to_string(fleet_path.join("environments").join("qa").join("fleet.nix")).unwrap();
    assert!(fleet_nix.contains(r#""srv-01" = {"#));
    assert!(fleet_nix.contains(r#"services = [ ];"#));
}
