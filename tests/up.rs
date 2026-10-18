mod common;

use std::fs;
use std::path::{Path, PathBuf};

use rocky::commands::up::{self, UpArgs};
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

fn setup_env_with_hosts(
    fleet_path: &Path,
    env_name: &str,
    hosts_content: &str,
    keys_content: Option<&str>,
) -> PathBuf {
    let env_dir = fleet_path.join("environments").join(env_name);
    fs::create_dir_all(&env_dir).unwrap();

    let fleet_nix = format!(
        r#"{{
  domain = "{env_name}.rocky.internal";
  hosts = {{
{hosts_content}
  }};
}}
"#
    );
    fs::write(env_dir.join("fleet.nix"), fleet_nix).unwrap();

    if let Some(keys) = keys_content {
        fs::write(env_dir.join("keys.nix"), keys).unwrap();
    }

    env_dir
}

#[tokio::test]
async fn test_up_rejects_non_fleet_directory() {
    let ws = common::TestWorkspace::new();
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
    let ws = common::TestWorkspace::new();
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
    let ws = common::TestWorkspace::new();
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
    let ws = common::TestWorkspace::new();
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
    let ws = common::TestWorkspace::new();
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
    let ws = common::TestWorkspace::new();
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
    let ws = common::TestWorkspace::new();
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
    let ws = common::TestWorkspace::new();
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
