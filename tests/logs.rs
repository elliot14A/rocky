mod common;

use std::fs;

use common::{setup_env_with_hosts, setup_fleet, TestWorkspace};
use rocky::commands::logs::{self, LogsArgs};
use rocky::commands::new::service::ServiceKind;
use rocky::process::journal::{
    build_journalctl_args, resolve_service_target, JournalOptions, ResolvedLogTarget,
    ResolvedService,
};

#[tokio::test]
async fn test_logs_rejects_nonexistent_environment() {
    let ws = TestWorkspace::new();
    let fleet_path = setup_fleet(&ws);

    let args = LogsArgs {
        env: "nonexistent".to_string(),
        service: None,
        host: None,
        follow: false,
        tail: None,
        since: None,
        until: None,
        timestamps: false,
    };

    let result = logs::run(args, Some(&fleet_path)).await;
    assert!(result.is_err());
    let err_str = result.unwrap_err().to_string();
    assert!(
        err_str.contains("Environment 'nonexistent' not found"),
        "expected environment not found error, got: {err_str}"
    );
}

#[tokio::test]
async fn test_logs_rejects_nonexistent_host() {
    let ws = TestWorkspace::new();
    let fleet_path = setup_fleet(&ws);

    let hosts_body = r#"    "db" = { services = [ ]; };"#;
    setup_env_with_hosts(&fleet_path, "prod", hosts_body, None);

    let args = LogsArgs {
        env: "prod".to_string(),
        service: None,
        host: Some("ghost".to_string()),
        follow: false,
        tail: None,
        since: None,
        until: None,
        timestamps: false,
    };

    let result = logs::run(args, Some(&fleet_path)).await;
    assert!(result.is_err());
    let err_str = result.unwrap_err().to_string();
    assert!(
        err_str.contains("Host 'ghost' not found"),
        "expected host not found error, got: {err_str}"
    );
}

#[tokio::test]
async fn test_logs_rejects_compound_env_host_target() {
    let ws = TestWorkspace::new();
    let fleet_path = setup_fleet(&ws);

    let hosts_body = r#"    "edge" = { services = [ ]; };"#;
    setup_env_with_hosts(&fleet_path, "prod", hosts_body, None);

    let args = LogsArgs {
        env: "prod-edge".to_string(),
        service: None,
        host: None,
        follow: false,
        tail: None,
        since: None,
        until: None,
        timestamps: false,
    };

    let result = logs::run(args, Some(&fleet_path)).await;
    assert!(result.is_err());
    let err_str = result.unwrap_err().to_string();
    assert!(
        err_str.contains("Environment 'prod-edge' not found"),
        "expected compound env-host to be rejected as environment, got: {err_str}"
    );
}

#[tokio::test]
async fn test_logs_rejects_nonexistent_service_in_environment() {
    let ws = TestWorkspace::new();
    let fleet_path = setup_fleet(&ws);

    let hosts_body = r#"    "db" = { services = [ "postgres" ]; };"#;
    setup_env_with_hosts(&fleet_path, "prod", hosts_body, None);

    let args = LogsArgs {
        env: "prod".to_string(),
        service: Some("ghost".to_string()),
        host: None,
        follow: false,
        tail: None,
        since: None,
        until: None,
        timestamps: false,
    };

    let result = logs::run(args, Some(&fleet_path)).await;
    assert!(result.is_err());
    let err_str = result.unwrap_err().to_string();
    assert!(
        err_str.contains("Service 'ghost' not found"),
        "expected service not found error, got: {err_str}"
    );
}

#[tokio::test]
async fn test_logs_rejects_service_missing_nix_file() {
    let ws = TestWorkspace::new();
    let fleet_path = setup_fleet(&ws);

    let hosts_body = r#"    "edge" = { services = [ "caddy" ]; };"#;
    setup_env_with_hosts(&fleet_path, "prod", hosts_body, None);

    let args = LogsArgs {
        env: "prod".to_string(),
        service: Some("caddy".to_string()),
        host: None,
        follow: false,
        tail: None,
        since: None,
        until: None,
        timestamps: false,
    };

    let result = logs::run(args, Some(&fleet_path)).await;
    assert!(result.is_err());
    let err_str = result.unwrap_err().to_string();
    assert!(
        err_str.contains("Service 'caddy' not found"),
        "expected missing service nix file error, got: {err_str}"
    );
}

#[tokio::test]
async fn test_logs_mesh_not_connected_fails_gracefully() {
    let ws = TestWorkspace::new();
    let fleet_path = setup_fleet(&ws);

    let hosts_body = r#"    "edge" = { services = [ "caddy" ]; };"#;
    setup_env_with_hosts(&fleet_path, "unjoined-env", hosts_body, None);

    let services_dir = fleet_path.join("services");
    fs::create_dir_all(&services_dir).unwrap();
    fs::write(
        services_dir.join("caddy.nix"),
        r#"{ services.caddy = { enable = true; }; }"#,
    )
    .unwrap();

    let args = LogsArgs {
        env: "unjoined-env".to_string(),
        service: Some("caddy".to_string()),
        host: None,
        follow: false,
        tail: Some(20),
        since: None,
        until: None,
        timestamps: false,
    };

    let result = logs::run(args, Some(&fleet_path)).await;
    assert!(result.is_err());
    let err_str = result.unwrap_err().to_string();
    assert!(
        err_str.contains("Workstation is not connected to fleet mesh 'unjoined-env'"),
        "expected mesh connection check failure, got: {err_str}"
    );
}

#[tokio::test]
async fn test_logs_target_resolution_podman_container() {
    let ws = TestWorkspace::new();
    let fleet_path = setup_fleet(&ws);

    let hosts_body = r#"
    "app-01" = {
      services = [ "gotrue" ];
    };
"#;
    setup_env_with_hosts(&fleet_path, "staging", hosts_body, None);

    let services_dir = fleet_path.join("services");
    fs::create_dir_all(&services_dir).unwrap();
    fs::write(
        services_dir.join("gotrue.nix"),
        r#"{ replicateContainers, ... }:
{
  virtualisation.oci-containers.containers = replicateContainers {
    service = "gotrue";
  };
}"#,
    )
    .unwrap();

    let target = resolve_service_target(&fleet_path, "staging", Some("gotrue"), None).unwrap();
    assert_eq!(target.host, "app-01");
    let svc = target.service.as_ref().expect("expected resolved service");
    assert_eq!(svc.name, "gotrue");
    assert_eq!(svc.kind, ServiceKind::Podman);
    assert_eq!(svc.journal_id, "podman-gotrue");

    let opts = JournalOptions {
        follow: true,
        tail: Some(50),
        timestamps: true,
        ..Default::default()
    };
    let args = build_journalctl_args(&target, &opts);
    assert_eq!(
        args,
        vec![
            "journalctl",
            "--no-pager",
            "-q",
            "-u",
            "podman-gotrue.service",
            "-u",
            "podman-gotrue-*.service",
            "-f",
            "-n",
            "50",
            "-o",
            "short-iso"
        ]
    );
}

#[tokio::test]
async fn test_logs_explicit_host_mismatch_rejected() {
    let ws = TestWorkspace::new();
    let fleet_path = setup_fleet(&ws);

    let hosts_body = r#"
    "edge" = { services = [ "caddy" ]; };
    "app" = { services = [ "gotrue" ]; };
"#;
    setup_env_with_hosts(&fleet_path, "prod", hosts_body, None);

    let services_dir = fleet_path.join("services");
    fs::create_dir_all(&services_dir).unwrap();
    fs::write(
        services_dir.join("caddy.nix"),
        r#"{ services.caddy = { enable = true; }; }"#,
    )
    .unwrap();
    fs::write(
        services_dir.join("gotrue.nix"),
        r#"{ virtualisation.oci-containers.containers.gotrue = {}; }"#,
    )
    .unwrap();

    let result = resolve_service_target(&fleet_path, "prod", Some("gotrue"), Some("edge"));
    assert!(result.is_err());
    let err_str = result.unwrap_err().to_string();
    assert!(
        err_str.contains("Service 'gotrue' not found"),
        "expected service not found on host error, got: {err_str}"
    );
}

#[tokio::test]
async fn test_logs_rejects_path_traversal_target() {
    let ws = TestWorkspace::new();
    let fleet_path = setup_fleet(&ws);

    let args = LogsArgs {
        env: "../../etc".to_string(),
        service: None,
        host: None,
        follow: false,
        tail: None,
        since: None,
        until: None,
        timestamps: false,
    };

    let result = logs::run(args, Some(&fleet_path)).await;
    assert!(result.is_err());
}

#[tokio::test]
async fn test_logs_rejects_path_traversal_service() {
    let ws = TestWorkspace::new();
    let fleet_path = setup_fleet(&ws);

    let hosts_body = r#"    "edge" = { services = [ "caddy" ]; };"#;
    setup_env_with_hosts(&fleet_path, "prod", hosts_body, None);

    let args = LogsArgs {
        env: "prod".to_string(),
        service: Some("../../etc/shadow".to_string()),
        host: None,
        follow: false,
        tail: None,
        since: None,
        until: None,
        timestamps: false,
    };

    let result = logs::run(args, Some(&fleet_path)).await;
    assert!(result.is_err());
}

#[test]
fn test_logs_shlex_quotes_remote_command() {
    let target = ResolvedLogTarget {
        host: "app".to_string(),
        service: Some(ResolvedService {
            name: "api".to_string(),
            kind: ServiceKind::Systemd,
            journal_id: "api".to_string(),
        }),
    };
    let opts = JournalOptions {
        follow: true,
        tail: Some(100),
        since: Some("1 hour ago; rm -rf /".to_string()),
        until: Some("now".to_string()),
        timestamps: true,
    };
    let args = build_journalctl_args(&target, &opts);
    let remote_cmd = shlex::try_join(args.iter().map(|s| s.as_str())).unwrap();

    // The semicolon and spaces must be properly quoted in the remote command string
    assert!(remote_cmd.contains("'1 hour ago; rm -rf /'"));
    assert!(remote_cmd.contains("-u api.service -u 'api-*.service'"));
}

#[tokio::test]
async fn test_logs_rejects_multi_host_environment_without_explicit_host() {
    let ws = TestWorkspace::new();
    let fleet_path = setup_fleet(&ws);

    let hosts_body = r#"
    "edge" = { services = [ "caddy" ]; };
    "app" = { services = [ "gotrue" ]; };
"#;
    setup_env_with_hosts(&fleet_path, "prod", hosts_body, None);

    let result = resolve_service_target(&fleet_path, "prod", None, None);
    assert!(result.is_err());
    let err_str = result.unwrap_err().to_string();
    assert!(
        err_str.contains("contains multiple hosts") && err_str.contains("specify target host"),
        "expected multi-host ambiguity error, got: {err_str}"
    );
}

#[tokio::test]
async fn test_logs_rejects_multi_host_service_without_explicit_host() {
    let ws = TestWorkspace::new();
    let fleet_path = setup_fleet(&ws);

    let hosts_body = r#"
    "web-01" = { services = [ "api" ]; };
    "web-02" = { services = [ "api" ]; };
"#;
    setup_env_with_hosts(&fleet_path, "prod", hosts_body, None);

    let services_dir = fleet_path.join("services");
    fs::create_dir_all(&services_dir).unwrap();
    fs::write(
        services_dir.join("api.nix"),
        r#"{ services.api = { enable = true; }; }"#,
    )
    .unwrap();

    let result = resolve_service_target(&fleet_path, "prod", Some("api"), None);
    assert!(result.is_err());
    let err_str = result.unwrap_err().to_string();
    assert!(
        err_str.contains("service 'api' is deployed on multiple hosts")
            && err_str.contains("specify target host with --host"),
        "expected service multi-host ambiguity error, got: {err_str}"
    );

    // When host is explicitly specified, it succeeds!
    let target = resolve_service_target(&fleet_path, "prod", Some("api"), Some("web-02")).unwrap();
    assert_eq!(target.host, "web-02");
    assert_eq!(target.service.unwrap().name, "api");
}

#[tokio::test]
async fn test_logs_rejects_control_characters_in_timestamp() {
    let ws = TestWorkspace::new();
    let fleet_path = setup_fleet(&ws);

    let args = LogsArgs {
        env: "prod".to_string(),
        service: None,
        host: None,
        follow: false,
        tail: None,
        since: Some("yesterday\x1b[31m".to_string()),
        until: None,
        timestamps: false,
    };

    let result = logs::run(args, Some(&fleet_path)).await;
    assert!(result.is_err());
    let err_str = result.unwrap_err().to_string();
    assert!(
        err_str.contains("contains invalid control characters"),
        "expected control character rejection, got: {err_str}"
    );
}
