mod common;

use std::fs;

use common::{setup_env, setup_fleet, setup_fleet_bare, TestWorkspace};
use rocky::commands::host::{self, new::HostArgs, HostArgs as CliHostArgs, HostCommand};
use rocky::error::{Kind, RockyError};

async fn run_host(args: HostArgs, target: Option<&std::path::Path>) -> rocky::error::Result<()> {
    host::run(
        CliHostArgs {
            command: HostCommand::New(args),
        },
        target,
    )
    .await
}

#[tokio::test]
async fn test_new_host_success_with_provider_and_env() {
    let ws = TestWorkspace::new();
    let fleet_path = setup_fleet(&ws);
    let env_dir = setup_env(&fleet_path, "prod");

    let args = HostArgs {
        name: "web-01".to_string(),
        env: Some("prod".to_string()),
        provider: Some("hetzner".to_string()),
        partition: None,
    };

    let res = run_host(args, Some(&fleet_path)).await;
    assert!(res.is_ok(), "host::run failed: {:?}", res);

    let host_dir = env_dir.join("hosts").join("web-01");
    assert!(host_dir.join("default.nix").is_file());
    assert!(host_dir.join("disko.nix").is_file());
    assert!(host_dir.join("hardware.nix").is_file());

    let default_nix = fs::read_to_string(host_dir.join("default.nix")).unwrap();
    assert!(default_nix.contains("prod-web-01"));

    let fleet_nix = fs::read_to_string(env_dir.join("fleet.nix")).unwrap();
    assert!(fleet_nix.contains(r#""web-01" = {"#));
    assert!(fleet_nix.contains(r#"services = [ ];"#));
}

#[tokio::test]
async fn test_new_host_oracle_provider() {
    let ws = TestWorkspace::new();
    let fleet_path = setup_fleet(&ws);
    let env_dir = setup_env(&fleet_path, "prod");

    let args = HostArgs {
        name: "arm-01".to_string(),
        env: Some("prod".to_string()),
        provider: Some("oracle".to_string()),
        partition: None,
    };

    let res = run_host(args, Some(&fleet_path)).await;
    assert!(res.is_ok(), "host::run failed: {:?}", res);

    let host_dir = env_dir.join("hosts").join("arm-01");
    assert!(host_dir.join("hardware.nix").is_file());
    let hardware_nix = fs::read_to_string(host_dir.join("hardware.nix")).unwrap();
    assert!(hardware_nix.contains("isAarch64"));
}

#[tokio::test]
async fn test_new_host_auto_selects_single_environment() {
    let ws = TestWorkspace::new();
    let fleet_path = setup_fleet(&ws);
    let env_dir = setup_env(&fleet_path, "staging");

    let args = HostArgs {
        name: "worker-01".to_string(),
        env: None,
        provider: Some("hetzner".to_string()),
        partition: None,
    };

    let res = run_host(args, Some(&fleet_path)).await;
    assert!(res.is_ok(), "host::run failed: {:?}", res);

    let host_dir = env_dir.join("hosts").join("worker-01");
    assert!(host_dir.join("default.nix").is_file());
    let default_nix = fs::read_to_string(host_dir.join("default.nix")).unwrap();
    assert!(default_nix.contains("staging-worker-01"));
}

#[tokio::test]
async fn test_new_host_rejects_ambiguous_environment_when_multiple_exist_and_none_specified() {
    let ws = TestWorkspace::new();
    let fleet_path = setup_fleet(&ws);
    setup_env(&fleet_path, "prod");
    setup_env(&fleet_path, "staging");

    let args = HostArgs {
        name: "worker-01".to_string(),
        env: None,
        provider: Some("hetzner".to_string()),
        partition: None,
    };

    let res = run_host(args, Some(&fleet_path)).await;
    match res {
        Err(RockyError::AmbiguousEnvironment { environments }) => {
            assert_eq!(
                environments,
                vec!["prod".to_string(), "staging".to_string()]
            );
        }
        other => panic!("expected AmbiguousEnvironment, got {:?}", other),
    }
}

#[tokio::test]
async fn test_new_host_rejects_already_existing_host() {
    let ws = TestWorkspace::new();
    let fleet_path = setup_fleet(&ws);
    setup_env(&fleet_path, "prod");

    let args1 = HostArgs {
        name: "db-01".to_string(),
        env: Some("prod".to_string()),
        provider: Some("hetzner".to_string()),
        partition: None,
    };
    run_host(args1, Some(&fleet_path)).await.unwrap();

    let args2 = HostArgs {
        name: "db-01".to_string(),
        env: Some("prod".to_string()),
        provider: Some("hetzner".to_string()),
        partition: None,
    };
    let res = run_host(args2, Some(&fleet_path)).await;
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
    let ws = TestWorkspace::new();
    let fleet_path = setup_fleet(&ws);
    setup_env(&fleet_path, "prod");

    let args = HostArgs {
        name: "../escape".to_string(),
        env: Some("prod".to_string()),
        provider: Some("hetzner".to_string()),
        partition: None,
    };

    let res = run_host(args, Some(&fleet_path)).await;
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
    let ws = TestWorkspace::new();
    let fleet_path = setup_fleet(&ws);
    setup_env(&fleet_path, "prod");

    let args = HostArgs {
        name: "node-01".to_string(),
        env: Some("prod".to_string()),
        provider: Some("unsupported-provider".to_string()),
        partition: None,
    };

    let res = run_host(args, Some(&fleet_path)).await;
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
    let ws = TestWorkspace::new();
    let fleet_path = setup_fleet(&ws);
    setup_env(&fleet_path, "prod");

    let args = HostArgs {
        name: "node-01".to_string(),
        env: Some("prod".to_string()),
        provider: Some("../escape".to_string()),
        partition: None,
    };

    let res = run_host(args, Some(&fleet_path)).await;
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
    let ws = TestWorkspace::new();
    let fleet_path = setup_fleet_bare(&ws);
    let env_dir = setup_env(&fleet_path, "prod");

    let args = HostArgs {
        name: "srv-01".to_string(),
        env: Some("prod".to_string()),
        provider: None,
        partition: None,
    };

    let res = run_host(args, Some(&fleet_path)).await;
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
    let ws = TestWorkspace::new();
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

    let res = run_host(args, Some(&fleet_path)).await;
    assert!(res.is_ok(), "host::run failed: {:?}", res);

    let host_dir = env_dir.join("hosts").join("srv-oracle");
    assert!(host_dir.join("hardware.nix").is_file());
    let hardware_nix = fs::read_to_string(host_dir.join("hardware.nix")).unwrap();
    assert!(hardware_nix.contains("isAarch64"));
}

#[tokio::test]
async fn test_new_host_gvm_provider() {
    let ws = TestWorkspace::new();
    let fleet_path = setup_fleet(&ws);
    let env_dir = setup_env(&fleet_path, "prod");

    let args = HostArgs {
        name: "gcp-01".to_string(),
        env: Some("prod".to_string()),
        provider: Some("gvm".to_string()),
        partition: None,
    };

    let res = run_host(args, Some(&fleet_path)).await;
    assert!(res.is_ok(), "host::run failed: {:?}", res);

    let host_dir = env_dir.join("hosts").join("gcp-01");
    assert!(host_dir.join("hardware.nix").is_file());
    let hardware_nix = fs::read_to_string(host_dir.join("hardware.nix")).unwrap();
    assert!(hardware_nix.contains("earlyprintk"));
}

#[tokio::test]
async fn test_new_host_rejects_host_already_declared_in_fleet_nix() {
    let ws = TestWorkspace::new();
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

    let res = run_host(args, Some(&fleet_path)).await;
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
    let ws = TestWorkspace::new();
    let fleet_path = setup_fleet(&ws);
    let env_dir = setup_env(&fleet_path, "prod");

    let args = HostArgs {
        name: "custom-disk-01".to_string(),
        env: Some("prod".to_string()),
        provider: Some("hetzner".to_string()),
        partition: Some("disko".to_string()),
    };

    let res = run_host(args, Some(&fleet_path)).await;
    assert!(res.is_ok(), "host::run failed: {:?}", res);

    let host_dir = env_dir.join("hosts").join("custom-disk-01");
    assert!(host_dir.join("default.nix").is_file());
    assert!(host_dir.join("disko.nix").is_file());
    assert!(host_dir.join("hardware.nix").is_file());
}

#[tokio::test]
async fn test_new_host_rejects_missing_fleet_nix() {
    let ws = TestWorkspace::new();
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

    let res = run_host(args, Some(&fleet_path)).await;
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
    let ws = TestWorkspace::new();
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

    let res = run_host(args, Some(&fleet_path)).await;
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

#[tokio::test]
async fn test_host_list_success() {
    let ws = TestWorkspace::new();
    let fleet_path = setup_fleet(&ws);
    let _env_dir = setup_env(&fleet_path, "prod");

    let args = HostArgs {
        name: "node-01".to_string(),
        env: Some("prod".to_string()),
        provider: Some("default".to_string()),
        partition: None,
    };
    run_host(args, Some(&fleet_path)).await.unwrap();

    let hosts = host::list::collect_hosts(&fleet_path, None).unwrap();
    assert_eq!(hosts.len(), 1);
    assert_eq!(hosts[0].name, "node-01");
    assert_eq!(hosts[0].environment, "prod");

    let res = host::run(
        CliHostArgs {
            command: HostCommand::List(host::list::ListArgs {
                env: None,
                env_flag: None,
            }),
        },
        Some(&fleet_path),
    )
    .await;
    assert!(res.is_ok());

    let hosts_filtered = host::list::collect_hosts(&fleet_path, Some("prod")).unwrap();
    assert_eq!(hosts_filtered.len(), 1);
    assert_eq!(hosts_filtered[0].name, "node-01");

    let res_filtered = host::run(
        CliHostArgs {
            command: HostCommand::List(host::list::ListArgs {
                env: Some("prod".to_string()),
                env_flag: None,
            }),
        },
        Some(&fleet_path),
    )
    .await;
    assert!(res_filtered.is_ok());
}

#[tokio::test]
async fn test_host_edit_missing_fails() {
    let ws = TestWorkspace::new();
    let fleet_path = setup_fleet(&ws);
    let _env_dir = setup_env(&fleet_path, "prod");

    let res = host::run(
        CliHostArgs {
            command: HostCommand::Edit(host::edit::EditArgs {
                name: "ghost".to_string(),
                env: Some("prod".to_string()),
            }),
        },
        Some(&fleet_path),
    )
    .await;

    match res {
        Err(RockyError::NotFound { kind, name, .. }) => {
            assert_eq!(kind, Kind::Host);
            assert_eq!(name, "ghost");
        }
        other => panic!("expected NotFound Host, got {:?}", other),
    }
}

#[tokio::test]
async fn test_host_edit_success_with_editor() {
    let ws = TestWorkspace::new();
    let fleet_path = setup_fleet(&ws);
    let env_dir = setup_env(&fleet_path, "prod");

    let args = HostArgs {
        name: "node-01".to_string(),
        env: Some("prod".to_string()),
        provider: Some("default".to_string()),
        partition: None,
    };
    run_host(args, Some(&fleet_path)).await.unwrap();

    let host_nix = env_dir.join("hosts").join("node-01").join("default.nix");

    // 1. Unchanged exit with /bin/true
    {
        let _e = common::ScopedEnvVar::set_editor("true").await;
        let res = host::run(
            CliHostArgs {
                command: HostCommand::Edit(host::edit::EditArgs {
                    name: "node-01".to_string(),
                    env: None,
                }),
            },
            Some(&fleet_path),
        )
        .await;
        assert!(res.is_ok());
    }

    // 2. Test actual file modification on disk
    {
        let script = common::create_mock_editor(&ws.path, "# verified host edit comment");
        let _e = common::ScopedEnvVar::set_editor(script.to_str().unwrap()).await;
        let res = host::run(
            CliHostArgs {
                command: HostCommand::Edit(host::edit::EditArgs {
                    name: "node-01".to_string(),
                    env: None,
                }),
            },
            Some(&fleet_path),
        )
        .await;
        assert!(res.is_ok());

        let content = fs::read_to_string(&host_nix).unwrap();
        assert!(content.contains("# verified host edit comment"));
    }

    // 3. Test syntax error rollback
    {
        let bad_script = common::create_syntax_error_editor(&ws.path);
        let _e = common::ScopedEnvVar::set_editor(bad_script.to_str().unwrap()).await;
        let res = host::run(
            CliHostArgs {
                command: HostCommand::Edit(host::edit::EditArgs {
                    name: "node-01".to_string(),
                    env: None,
                }),
            },
            Some(&fleet_path),
        )
        .await;
        assert!(matches!(res, Err(RockyError::NixParse { .. })));

        let content = fs::read_to_string(&host_nix).unwrap();
        assert!(!content.contains("broken nix = {"));
    }
}
