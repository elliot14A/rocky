mod common;

use std::fs;

use common::{setup_fleet, TestWorkspace};
use rocky::commands::new::env::{self, EnvArgs};
use rocky::error::{Kind, RockyError};

#[tokio::test]
async fn test_new_env_success() {
    let ws = TestWorkspace::new();
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
    let ws = TestWorkspace::new();
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
    let ws = TestWorkspace::new();
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
    let ws = TestWorkspace::new();
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
