mod common;

use std::fs;

use common::{setup_fleet, TestWorkspace};
use rocky::commands::env::{self, new::NewArgs, EnvArgs, EnvCommand};
use rocky::error::{Kind, RockyError};

fn new_env_args(name: &str) -> EnvArgs {
    EnvArgs {
        command: EnvCommand::New(NewArgs {
            name: name.to_string(),
        }),
    }
}

#[tokio::test]
async fn test_new_env_success() {
    let ws = TestWorkspace::new();
    let fleet_path = setup_fleet(&ws);

    let args = new_env_args("staging");

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

    let args1 = new_env_args("prod");
    let res1 = env::run(args1, Some(&fleet_path)).await;
    assert!(res1.is_ok());

    let args2 = new_env_args("prod");
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

    let args = new_env_args("../evil");
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

    let args = new_env_args("prod");
    let res = env::run(args, Some(&empty_path)).await;
    match res {
        Err(RockyError::NotFound { kind, .. }) => {
            assert_eq!(kind, Kind::Fleet);
        }
        other => panic!("expected NotFound Fleet, got {:?}", other),
    }
}

#[tokio::test]
async fn test_env_list_success() {
    let ws = TestWorkspace::new();
    let fleet_path = setup_fleet(&ws);

    let envs_empty = env::list::collect_environments(&fleet_path).unwrap();
    assert!(envs_empty.is_empty());

    let res = env::run(
        EnvArgs {
            command: EnvCommand::List(env::list::ListArgs {}),
        },
        Some(&fleet_path),
    )
    .await;
    assert!(res.is_ok());

    // Create environments and list again
    env::run(new_env_args("prod"), Some(&fleet_path))
        .await
        .unwrap();
    env::run(new_env_args("staging"), Some(&fleet_path))
        .await
        .unwrap();

    let envs = env::list::collect_environments(&fleet_path).unwrap();
    assert_eq!(envs.len(), 2);
    assert_eq!(envs[0].name, "prod");
    assert_eq!(envs[1].name, "staging");

    let res2 = env::run(
        EnvArgs {
            command: EnvCommand::List(env::list::ListArgs {}),
        },
        Some(&fleet_path),
    )
    .await;
    assert!(res2.is_ok());
}

#[tokio::test]
async fn test_env_edit_missing_fails() {
    let ws = TestWorkspace::new();
    let fleet_path = setup_fleet(&ws);

    let res = env::run(
        EnvArgs {
            command: EnvCommand::Edit(env::edit::EditArgs {
                name: "missing".to_string(),
            }),
        },
        Some(&fleet_path),
    )
    .await;

    match res {
        Err(RockyError::NotFound { kind, name, .. }) => {
            assert_eq!(kind, Kind::Environment);
            assert_eq!(name, "missing");
        }
        other => panic!("expected NotFound Environment, got {:?}", other),
    }
}

#[tokio::test]
async fn test_env_edit_success_with_editor() {
    let ws = TestWorkspace::new();
    let fleet_path = setup_fleet(&ws);

    env::run(new_env_args("prod"), Some(&fleet_path))
        .await
        .unwrap();

    let fleet_nix_path = fleet_path
        .join("environments")
        .join("prod")
        .join("fleet.nix");

    // 1. Unchanged exit with /bin/true
    {
        let _e = common::ScopedEnvVar::set_editor("true").await;
        let res = env::run(
            EnvArgs {
                command: EnvCommand::Edit(env::edit::EditArgs {
                    name: "prod".to_string(),
                }),
            },
            Some(&fleet_path),
        )
        .await;
        assert!(res.is_ok());
    }

    // 2. Test actual file modification on disk
    {
        let script = common::create_mock_editor(&ws.path, "# verified env edit comment");
        let _e = common::ScopedEnvVar::set_editor(script.to_str().unwrap()).await;
        let res = env::run(
            EnvArgs {
                command: EnvCommand::Edit(env::edit::EditArgs {
                    name: "prod".to_string(),
                }),
            },
            Some(&fleet_path),
        )
        .await;
        assert!(res.is_ok());

        let content = fs::read_to_string(&fleet_nix_path).unwrap();
        assert!(content.contains("# verified env edit comment"));
    }

    // 3. Test syntax error rollback
    {
        let bad_script = common::create_syntax_error_editor(&ws.path);
        let _e = common::ScopedEnvVar::set_editor(bad_script.to_str().unwrap()).await;
        let res = env::run(
            EnvArgs {
                command: EnvCommand::Edit(env::edit::EditArgs {
                    name: "prod".to_string(),
                }),
            },
            Some(&fleet_path),
        )
        .await;
        assert!(matches!(res, Err(RockyError::NixParse { .. })));

        let content = fs::read_to_string(&fleet_nix_path).unwrap();
        assert!(!content.contains("broken nix = {"));
    }
}
