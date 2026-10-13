mod common;

use std::fs;
use std::path::PathBuf;

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
async fn test_new_host_success_with_provider_and_env() {
    let ws = common::TestWorkspace::new();
    let fleet_path = setup_fleet(&ws);
    let env_dir = fleet_path.join("environments").join("prod");
    fs::create_dir_all(&env_dir).unwrap();

    let args = HostArgs {
        name: "web-01".to_string(),
        env: Some("prod".to_string()),
        provider: Some("hetzner".to_string()),
    };

    let res = host::run(args, Some(&fleet_path)).await;
    assert!(res.is_ok(), "host::run failed: {:?}", res);

    let host_dir = env_dir.join("hosts").join("web-01");
    assert!(host_dir.join("default.nix").is_file());
    assert!(host_dir.join("disko.nix").is_file());
    assert!(host_dir.join("hardware.nix").is_file());

    let default_nix = fs::read_to_string(host_dir.join("default.nix")).unwrap();
    assert!(default_nix.contains("prod-web-01"));
}

#[tokio::test]
async fn test_new_host_oracle_provider() {
    let ws = common::TestWorkspace::new();
    let fleet_path = setup_fleet(&ws);
    let env_dir = fleet_path.join("environments").join("prod");
    fs::create_dir_all(&env_dir).unwrap();

    let args = HostArgs {
        name: "arm-01".to_string(),
        env: Some("prod".to_string()),
        provider: Some("oracle".to_string()),
    };

    let res = host::run(args, Some(&fleet_path)).await;
    assert!(res.is_ok(), "host::run failed: {:?}", res);

    let host_dir = env_dir.join("hosts").join("arm-01");
    assert!(host_dir.join("hardware.nix").is_file());
    let hardware_nix = fs::read_to_string(host_dir.join("hardware.nix")).unwrap();
    assert!(hardware_nix.contains("isAarch64"));
}

#[tokio::test]
async fn test_new_host_auto_selects_single_environment() {
    let ws = common::TestWorkspace::new();
    let fleet_path = setup_fleet(&ws);
    let env_dir = fleet_path.join("environments").join("staging");
    fs::create_dir_all(&env_dir).unwrap();

    let args = HostArgs {
        name: "worker-01".to_string(),
        env: None,
        provider: Some("hetzner".to_string()),
    };

    let res = host::run(args, Some(&fleet_path)).await;
    assert!(res.is_ok(), "host::run failed: {:?}", res);

    let host_dir = env_dir.join("hosts").join("worker-01");
    assert!(host_dir.join("default.nix").is_file());
    let default_nix = fs::read_to_string(host_dir.join("default.nix")).unwrap();
    assert!(default_nix.contains("staging-worker-01"));
}

#[tokio::test]
async fn test_new_host_picks_first_environment_when_multiple_exist_and_none_specified() {
    let ws = common::TestWorkspace::new();
    let fleet_path = setup_fleet(&ws);
    fs::create_dir_all(fleet_path.join("environments").join("prod")).unwrap();
    fs::create_dir_all(fleet_path.join("environments").join("staging")).unwrap();

    let args = HostArgs {
        name: "worker-01".to_string(),
        env: None,
        provider: Some("hetzner".to_string()),
    };

    let res = host::run(args, Some(&fleet_path)).await;
    assert!(res.is_ok(), "expected host::run to succeed, got: {:?}", res);

    let host_dir = fleet_path
        .join("environments")
        .join("prod")
        .join("hosts")
        .join("worker-01");
    assert!(host_dir.join("default.nix").is_file());
    let default_nix = fs::read_to_string(host_dir.join("default.nix")).unwrap();
    assert!(default_nix.contains("prod-worker-01"));
}

#[tokio::test]
async fn test_new_host_rejects_already_existing_host() {
    let ws = common::TestWorkspace::new();
    let fleet_path = setup_fleet(&ws);
    let env_dir = fleet_path.join("environments").join("prod");
    fs::create_dir_all(&env_dir).unwrap();

    let args1 = HostArgs {
        name: "db-01".to_string(),
        env: Some("prod".to_string()),
        provider: Some("hetzner".to_string()),
    };
    host::run(args1, Some(&fleet_path)).await.unwrap();

    let args2 = HostArgs {
        name: "db-01".to_string(),
        env: Some("prod".to_string()),
        provider: Some("hetzner".to_string()),
    };
    let res = host::run(args2, Some(&fleet_path)).await;
    match res {
        Err(RockyError::AlreadyExists { kind, name, .. }) => {
            assert_eq!(kind, Kind::Host);
            assert_eq!(name, "db-01");
        }
        other => panic!("expected AlreadyExists, got {:?}", other),
    }
}

#[tokio::test]
async fn test_new_host_rejects_invalid_name() {
    let ws = common::TestWorkspace::new();
    let fleet_path = setup_fleet(&ws);
    let env_dir = fleet_path.join("environments").join("prod");
    fs::create_dir_all(&env_dir).unwrap();

    let args = HostArgs {
        name: "../escape".to_string(),
        env: Some("prod".to_string()),
        provider: Some("hetzner".to_string()),
    };

    let res = host::run(args, Some(&fleet_path)).await;
    match res {
        Err(RockyError::InvalidName { kind, name }) => {
            assert_eq!(kind, Kind::Host);
            assert_eq!(name, "../escape");
        }
        other => panic!("expected InvalidName, got {:?}", other),
    }
}

#[tokio::test]
async fn test_new_host_rejects_unsupported_provider() {
    let ws = common::TestWorkspace::new();
    let fleet_path = setup_fleet(&ws);
    let env_dir = fleet_path.join("environments").join("prod");
    fs::create_dir_all(&env_dir).unwrap();

    let args = HostArgs {
        name: "node-01".to_string(),
        env: Some("prod".to_string()),
        provider: Some("unsupported-provider".to_string()),
    };

    let res = host::run(args, Some(&fleet_path)).await;
    match res {
        Err(RockyError::NotFound { kind, name, .. }) => {
            assert_eq!(kind, Kind::Provider);
            assert_eq!(name, "unsupported-provider");
        }
        other => panic!("expected NotFound Provider, got {:?}", other),
    }
}

#[tokio::test]
async fn test_new_host_rejects_invalid_provider_name() {
    let ws = common::TestWorkspace::new();
    let fleet_path = setup_fleet(&ws);
    let env_dir = fleet_path.join("environments").join("prod");
    fs::create_dir_all(&env_dir).unwrap();

    let args = HostArgs {
        name: "node-01".to_string(),
        env: Some("prod".to_string()),
        provider: Some("../escape".to_string()),
    };

    let res = host::run(args, Some(&fleet_path)).await;
    match res {
        Err(RockyError::InvalidName { kind, name }) => {
            assert_eq!(kind, Kind::Provider);
            assert_eq!(name, "../escape");
        }
        other => panic!("expected InvalidName Provider, got {:?}", other),
    }
}

#[tokio::test]
async fn test_new_host_fails_when_default_provider_missing() {
    let ws = common::TestWorkspace::new();
    let fleet_path = ws.path.join("rocky");
    let env_dir = fleet_path.join("environments").join("prod");
    fs::create_dir_all(&env_dir).unwrap();

    // Create a flake.nix with rockyFleet = true, but without config.defaultProvider
    fs::write(
        fleet_path.join("flake.nix"),
        r#"{
  description = "rocky test";
  outputs = inputs: {
    rockyFleet = true;
  };
}
"#,
    )
    .unwrap();

    let args = HostArgs {
        name: "srv-01".to_string(),
        env: Some("prod".to_string()),
        provider: None,
    };

    let res = host::run(args, Some(&fleet_path)).await;
    match res {
        Err(RockyError::NotFound { kind, name, .. }) => {
            assert_eq!(kind, Kind::Provider);
            assert_eq!(name, "defaultProvider");
        }
        other => panic!("expected NotFound defaultProvider, got {:?}", other),
    }
}

#[tokio::test]
async fn test_new_host_auto_detects_default_provider_from_flake() {
    let ws = common::TestWorkspace::new();
    let fleet_path = ws.path.join("rocky");
    let env_dir = fleet_path.join("environments").join("prod");
    fs::create_dir_all(&env_dir).unwrap();

    // Create a flake.nix with rockyFleet = true and config.defaultProvider = "oracle"
    fs::write(
        fleet_path.join("flake.nix"),
        r#"{
  description = "test";
  outputs = inputs: {
    rockyFleet = true;
    config = {
      defaultProvider = "oracle";
    };
  };
}
"#,
    )
    .unwrap();

    let args = HostArgs {
        name: "srv-oracle".to_string(),
        env: Some("prod".to_string()),
        provider: None,
    };

    let res = host::run(args, Some(&fleet_path)).await;
    assert!(res.is_ok(), "host::run failed: {:?}", res);

    let host_dir = env_dir.join("hosts").join("srv-oracle");
    assert!(host_dir.join("hardware.nix").is_file());
    let hardware_nix = fs::read_to_string(host_dir.join("hardware.nix")).unwrap();
    assert!(hardware_nix.contains("isAarch64"));
}
