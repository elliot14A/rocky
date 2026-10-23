mod common;

use std::fs;

use common::{setup_env_with_hosts, setup_fleet, TestWorkspace};
use rocky::commands::up::{self, UpArgs};
use rocky::error::{Kind, RockyError};

#[tokio::test]
async fn test_up_rejects_non_fleet_directory() {
    let ws = TestWorkspace::new();
    let non_fleet_path = ws.path.join("random_dir");
    fs::create_dir_all(&non_fleet_path).unwrap();

    let args = UpArgs {
        environment: "prod".to_string(),
        target: Vec::new(),
        identity: None,
        yes: true,
        non_interactive: true,
        concurrency: None,
    };

    let res = up::run(args, Some(&non_fleet_path)).await;
    match res {
        Err(RockyError::NotFound { kind, .. }) => {
            assert_eq!(kind, Kind::Fleet);
        }
        other => panic!("expected NotFound(Kind::Fleet), got: {:?}", other),
    }
}

#[tokio::test]
async fn test_up_rejects_missing_environment() {
    let ws = TestWorkspace::new();
    let fleet_path = setup_fleet(&ws);

    let args = UpArgs {
        environment: "nonexistent-env".to_string(),
        target: Vec::new(),
        identity: None,
        yes: true,
        non_interactive: true,
        concurrency: None,
    };

    let res = up::run(args, Some(&fleet_path)).await;
    match res {
        Err(RockyError::NotFound { kind, name, .. }) => {
            assert_eq!(kind, Kind::Environment);
            assert_eq!(name, "nonexistent-env");
        }
        other => panic!("expected NotFound(Kind::Environment), got: {:?}", other),
    }
}

#[tokio::test]
async fn test_up_rejects_missing_fleet_nix() {
    let ws = TestWorkspace::new();
    let fleet_path = setup_fleet(&ws);

    let env_dir = fleet_path.join("environments").join("broken-env");
    fs::create_dir_all(&env_dir).unwrap();

    let args = UpArgs {
        environment: "broken-env".to_string(),
        target: Vec::new(),
        identity: None,
        yes: true,
        non_interactive: true,
        concurrency: None,
    };

    let res = up::run(args, Some(&fleet_path)).await;
    match res {
        Err(RockyError::NotFound { kind, name, .. }) => {
            assert_eq!(kind, Kind::Fleet);
            assert_eq!(name, "fleet.nix");
        }
        other => panic!("expected NotFound(Kind::Fleet), got: {:?}", other),
    }
}

#[tokio::test]
async fn test_up_empty_fleet_is_up_to_date() {
    let ws = TestWorkspace::new();
    let fleet_path = setup_fleet(&ws);
    setup_env_with_hosts(&fleet_path, "prod", "", None);

    let args = UpArgs {
        environment: "prod".to_string(),
        target: Vec::new(),
        identity: None,
        yes: true,
        non_interactive: true,
        concurrency: None,
    };

    let res = up::run(args, Some(&fleet_path)).await;
    assert!(res.is_ok(), "up::run on empty fleet failed: {:?}", res);
}

#[tokio::test]
async fn test_up_fails_on_pending_host_without_target_ip() {
    let ws = TestWorkspace::new();
    let fleet_path = setup_fleet(&ws);
    setup_env_with_hosts(
        &fleet_path,
        "staging",
        r#"    "node-01" = { services = [ "web" ]; };"#,
        None,
    );

    let args = UpArgs {
        environment: "staging".to_string(),
        target: Vec::new(),
        identity: None,
        yes: true,
        non_interactive: true,
        concurrency: None,
    };

    let res = up::run(args, Some(&fleet_path)).await;
    match res {
        Err(RockyError::MissingTargetIp { host }) => {
            assert_eq!(host, "node-01");
        }
        other => panic!("expected MissingTargetIp(node-01), got: {:?}", other),
    }
}

#[tokio::test]
async fn test_up_fails_on_invalid_target_syntax() {
    let ws = TestWorkspace::new();
    let fleet_path = setup_fleet(&ws);
    setup_env_with_hosts(
        &fleet_path,
        "staging",
        r#"    "node-01" = { services = [ ]; };"#,
        None,
    );

    let args = UpArgs {
        environment: "staging".to_string(),
        target: vec!["invalid-syntax-no-equals".to_string()],
        identity: None,
        yes: true,
        non_interactive: true,
        concurrency: None,
    };

    let res = up::run(args, Some(&fleet_path)).await;
    match res {
        Err(RockyError::TargetParse { raw, .. }) => {
            assert_eq!(raw, "invalid-syntax-no-equals");
        }
        other => panic!("expected TargetParse, got: {:?}", other),
    }
}

#[tokio::test]
async fn test_up_fails_when_target_specifies_wrong_host() {
    let ws = TestWorkspace::new();
    let fleet_path = setup_fleet(&ws);
    setup_env_with_hosts(
        &fleet_path,
        "staging",
        r#"    "node-01" = { services = [ ]; };"#,
        None,
    );

    let args = UpArgs {
        environment: "staging".to_string(),
        target: vec!["other-node=192.168.1.10".to_string()],
        identity: None,
        yes: true,
        non_interactive: true,
        concurrency: None,
    };

    let res = up::run(args, Some(&fleet_path)).await;
    match res {
        Err(RockyError::MissingTargetIp { host }) => {
            assert_eq!(host, "node-01");
        }
        other => panic!("expected MissingTargetIp(node-01), got: {:?}", other),
    }
}

#[tokio::test]
async fn test_up_aborts_when_unconfirmed_in_non_interactive_mode() {
    let ws = TestWorkspace::new();
    let fleet_path = setup_fleet(&ws);
    setup_env_with_hosts(
        &fleet_path,
        "staging",
        r#"    "node-01" = { services = [ ]; };"#,
        None,
    );

    let args = UpArgs {
        environment: "staging".to_string(),
        target: vec!["node-01=192.168.1.10".to_string()],
        identity: None,
        yes: false, // unconfirmed
        non_interactive: true,
        concurrency: None,
    };

    let res = up::run(args, Some(&fleet_path)).await;
    assert!(
        res.is_ok(),
        "expected aborted deployment to return Ok(()), got: {:?}",
        res
    );

    // Verify no keys were registered because plan was aborted
    let keys_path = fleet_path
        .join("environments")
        .join("staging")
        .join("keys.nix");
    assert!(!keys_path.exists());
}

#[test]
fn test_up_comma_separated_targets_parsing() {
    use clap::Parser;
    use rocky::commands::{Cli, Command};

    let cli = Cli::try_parse_from([
        "rocky",
        "up",
        "prod",
        "-t",
        "node-01=192.168.1.10:2222,node-02=192.168.1.11:2222",
    ])
    .unwrap();

    match cli.command {
        Command::Up(args) => {
            assert_eq!(
                args.target,
                vec!["node-01=192.168.1.10:2222", "node-02=192.168.1.11:2222"]
            );
        }
        _ => panic!("unexpected command parsed"),
    }
}

#[test]
fn test_up_mixed_and_long_form_targets_parsing() {
    use clap::Parser;
    use rocky::commands::{Cli, Command};

    let cli = Cli::try_parse_from([
        "rocky",
        "up",
        "prod",
        "-t",
        "node-01=192.168.1.10:2222,node-02=192.168.1.11:2222",
        "--target",
        "node-03=192.168.1.12:2222",
    ])
    .unwrap();

    match cli.command {
        Command::Up(args) => {
            assert_eq!(
                args.target,
                vec![
                    "node-01=192.168.1.10:2222",
                    "node-02=192.168.1.11:2222",
                    "node-03=192.168.1.12:2222",
                ]
            );
        }
        _ => panic!("unexpected command parsed"),
    }
}

#[test]
fn test_up_parse_targets_with_comma_separated_inputs() {
    use clap::Parser;
    use rocky::commands::{Cli, Command};
    use rocky::ssh::parse_targets;

    let cli = Cli::try_parse_from([
        "rocky",
        "up",
        "prod",
        "-t",
        "node-01=192.168.1.10:2222,node-02=192.168.1.11:2223",
    ])
    .unwrap();

    let targets = match cli.command {
        Command::Up(args) => parse_targets(&args.target).unwrap(),
        _ => panic!("unexpected command parsed"),
    };

    assert_eq!(targets.len(), 2);
    let t1 = targets.get("node-01").unwrap();
    assert_eq!(t1.address, "192.168.1.10");
    assert_eq!(t1.port, 2222);

    let t2 = targets.get("node-02").unwrap();
    assert_eq!(t2.address, "192.168.1.11");
    assert_eq!(t2.port, 2223);
}
