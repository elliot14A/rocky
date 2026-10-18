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

fn setup_env(fleet_path: &std::path::Path, env_name: &str) -> PathBuf {
    let env_dir = fleet_path.join("environments").join(env_name);
    fs::create_dir_all(&env_dir).unwrap();
    fs::write(env_dir.join("fleet.nix"), "{\n  hosts = { };\n}\n").unwrap();
    env_dir
}

#[tokio::test]
async fn test_new_host_success_with_provider_and_env() {
    let ws = common::TestWorkspace::new();
    let fleet_path = setup_fleet(&ws);
    let env_dir = setup_env(&fleet_path, "prod");

    let args = HostArgs {
        name: "web-01".to_string(),
        env: Some("prod".to_string()),
        provider: Some("hetzner".to_string()),
        partition: None,
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
    let env_dir = setup_env(&fleet_path, "prod");

    let args = HostArgs {
        name: "arm-01".to_string(),
        env: Some("prod".to_string()),
        provider: Some("oracle".to_string()),
        partition: None,
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
    let env_dir = setup_env(&fleet_path, "staging");

    let args = HostArgs {
        name: "worker-01".to_string(),
        env: None,
        provider: Some("hetzner".to_string()),
        partition: None,
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
    setup_env(&fleet_path, "prod");
    setup_env(&fleet_path, "staging");

    let args = HostArgs {
        name: "worker-01".to_string(),
        env: None,
        provider: Some("hetzner".to_string()),
        partition: None,
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
    setup_env(&fleet_path, "prod");

    let args1 = HostArgs {
        name: "db-01".to_string(),
        env: Some("prod".to_string()),
        provider: Some("hetzner".to_string()),
        partition: None,
    };
    host::run(args1, Some(&fleet_path)).await.unwrap();

    let args2 = HostArgs {
        name: "db-01".to_string(),
        env: Some("prod".to_string()),
        provider: Some("hetzner".to_string()),
        partition: None,
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
    setup_env(&fleet_path, "prod");

    let args = HostArgs {
        name: "../escape".to_string(),
        env: Some("prod".to_string()),
        provider: Some("hetzner".to_string()),
        partition: None,
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
    setup_env(&fleet_path, "prod");

    let args = HostArgs {
        name: "node-01".to_string(),
        env: Some("prod".to_string()),
        provider: Some("unsupported-provider".to_string()),
        partition: None,
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
    setup_env(&fleet_path, "prod");

    let args = HostArgs {
        name: "node-01".to_string(),
        env: Some("prod".to_string()),
        provider: Some("../escape".to_string()),
        partition: None,
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
async fn test_new_host_falls_back_to_default_when_provider_and_default_provider_absent() {
    let ws = common::TestWorkspace::new();
    let fleet_path = ws.path.join("rocky");
    let env_dir = setup_env(&fleet_path, "prod");

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
        partition: None,
    };

    let res = host::run(args, Some(&fleet_path)).await;
    assert!(res.is_ok(), "host::run failed: {:?}", res);

    let host_dir = env_dir.join("hosts").join("srv-01");
    assert!(host_dir.join("default.nix").is_file());
    assert!(host_dir.join("disko.nix").is_file());
    assert!(host_dir.join("hardware.nix").is_file());

    let hardware_nix = fs::read_to_string(host_dir.join("hardware.nix")).unwrap();
    assert!(hardware_nix.contains("serialPort = if pkgs.stdenv.hostPlatform.isAarch64"));
}

#[tokio::test]
async fn test_new_host_auto_detects_default_provider_from_flake() {
    let ws = common::TestWorkspace::new();
    let fleet_path = ws.path.join("rocky");
    let env_dir = setup_env(&fleet_path, "prod");

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
        partition: None,
    };

    let res = host::run(args, Some(&fleet_path)).await;
    assert!(res.is_ok(), "host::run failed: {:?}", res);

    let host_dir = env_dir.join("hosts").join("srv-oracle");
    assert!(host_dir.join("hardware.nix").is_file());
    let hardware_nix = fs::read_to_string(host_dir.join("hardware.nix")).unwrap();
    assert!(hardware_nix.contains("isAarch64"));
}

#[tokio::test]
async fn test_new_host_gvm_provider() {
    let ws = common::TestWorkspace::new();
    let fleet_path = setup_fleet(&ws);
    let env_dir = setup_env(&fleet_path, "prod");

    let args = HostArgs {
        name: "gcp-01".to_string(),
        env: Some("prod".to_string()),
        provider: Some("gvm".to_string()),
        partition: None,
    };

    let res = host::run(args, Some(&fleet_path)).await;
    assert!(res.is_ok(), "host::run failed: {:?}", res);

    let host_dir = env_dir.join("hosts").join("gcp-01");
    assert!(host_dir.join("hardware.nix").is_file());
    let hardware_nix = fs::read_to_string(host_dir.join("hardware.nix")).unwrap();
    assert!(hardware_nix.contains("earlyprintk"));
}

#[tokio::test]
async fn test_new_host_registers_in_fleet_nix() {
    let ws = common::TestWorkspace::new();
    let fleet_path = setup_fleet(&ws);
    let env_dir = setup_env(&fleet_path, "prod");

    let initial_fleet_nix = r#"{
  settings = {
    domain = "prod.rocky.internal";
  };
  hosts = { };
}
"#;
    fs::write(env_dir.join("fleet.nix"), initial_fleet_nix).unwrap();

    let args = HostArgs {
        name: "node-01".to_string(),
        env: Some("prod".to_string()),
        provider: Some("hetzner".to_string()),
        partition: None,
    };

    let res = host::run(args, Some(&fleet_path)).await;
    assert!(res.is_ok(), "host::run failed: {:?}", res);

    let updated_fleet_nix = fs::read_to_string(env_dir.join("fleet.nix")).unwrap();
    assert!(updated_fleet_nix.contains(r#""node-01" = {"#));
    assert!(updated_fleet_nix.contains(r#"services = [ ];"#));
}

#[tokio::test]
async fn test_new_host_rejects_host_already_declared_in_fleet_nix() {
    let ws = common::TestWorkspace::new();
    let fleet_path = setup_fleet(&ws);
    let env_dir = setup_env(&fleet_path, "prod");

    let fleet_nix_with_host = r#"{
  hosts = {
    "node-01" = {
      services = [ ];
    };
  };
}
"#;
    fs::write(env_dir.join("fleet.nix"), fleet_nix_with_host).unwrap();

    let args = HostArgs {
        name: "node-01".to_string(),
        env: Some("prod".to_string()),
        provider: Some("hetzner".to_string()),
        partition: None,
    };

    let res = host::run(args, Some(&fleet_path)).await;
    match res {
        Err(RockyError::AlreadyExists { kind, name, .. }) => {
            assert_eq!(kind, Kind::Host);
            assert_eq!(name, "node-01");
        }
        other => panic!("expected AlreadyExists Host, got {:?}", other),
    }

    // Host dir should not be created / left behind
    assert!(!env_dir.join("hosts").join("node-01").exists());
}

#[tokio::test]
async fn test_new_host_with_partition_flag() {
    let ws = common::TestWorkspace::new();
    let fleet_path = setup_fleet(&ws);
    let env_dir = setup_env(&fleet_path, "prod");

    let args = HostArgs {
        name: "custom-disk-01".to_string(),
        env: Some("prod".to_string()),
        provider: Some("hetzner".to_string()),
        partition: Some("disko".to_string()),
    };

    let res = host::run(args, Some(&fleet_path)).await;
    assert!(res.is_ok(), "host::run failed: {:?}", res);

    let host_dir = env_dir.join("hosts").join("custom-disk-01");
    assert!(host_dir.join("default.nix").is_file());
    assert!(host_dir.join("disko.nix").is_file());
    assert!(host_dir.join("hardware.nix").is_file());
}

#[tokio::test]
async fn test_new_host_rejects_missing_fleet_nix() {
    let ws = common::TestWorkspace::new();
    let fleet_path = setup_fleet(&ws);
    let env_dir = fleet_path.join("environments").join("prod");
    fs::create_dir_all(&env_dir).unwrap();
    // Intentionally no fleet.nix created

    let args = HostArgs {
        name: "node-01".to_string(),
        env: Some("prod".to_string()),
        provider: Some("hetzner".to_string()),
        partition: None,
    };

    let res = host::run(args, Some(&fleet_path)).await;
    match res {
        Err(RockyError::NotFound { kind, name, .. }) => {
            assert_eq!(kind, Kind::Fleet);
            assert_eq!(name, "fleet.nix");
        }
        other => panic!("expected NotFound Fleet, got {:?}", other),
    }

    // Host dir should not be created
    assert!(!env_dir.join("hosts").join("node-01").exists());
}

#[tokio::test]
async fn test_new_host_preserves_comments_and_custom_settings() {
    let ws = common::TestWorkspace::new();
    let fleet_path = setup_fleet(&ws);
    let env_dir = fleet_path.join("environments").join("prod");
    fs::create_dir_all(&env_dir).unwrap();

    let initial_fleet = r#"# Fleet configuration for production
{
  # Custom fleet settings
  settings = {
    domain = "rocky.corp";
  };

  # Active fleet hosts
  hosts = {
    # Bastion host
    "bastion-01" = {
      services = [ "ssh" ];
    };
  };
}
"#;
    fs::write(env_dir.join("fleet.nix"), initial_fleet).unwrap();

    let args = HostArgs {
        name: "worker-01".to_string(),
        env: Some("prod".to_string()),
        provider: Some("hetzner".to_string()),
        partition: None,
    };

    let res = host::run(args, Some(&fleet_path)).await;
    assert!(res.is_ok(), "host::run failed: {:?}", res);

    let updated_fleet = fs::read_to_string(env_dir.join("fleet.nix")).unwrap();

    // Verify comments remain intact
    assert!(updated_fleet.contains("# Fleet configuration for production"));
    assert!(updated_fleet.contains("# Custom fleet settings"));
    assert!(updated_fleet.contains("# Active fleet hosts"));
    assert!(updated_fleet.contains("# Bastion host"));

    // Verify existing settings and both hosts are present
    assert!(updated_fleet.contains("rocky.corp"));
    assert!(updated_fleet.contains(r#""bastion-01""#));
    assert!(updated_fleet.contains(r#""worker-01""#));
}
