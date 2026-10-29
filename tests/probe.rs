use rocky::error::{Kind, RockyError};
use rocky::ssh::{build_probe_args, is_tailscale_online, probe_tailscale_connectivity};

#[tokio::test]
async fn test_probe_fails_on_unreachable_target() {
    let res = probe_tailscale_connectivity("prod", "edge", "", "127.0.0.1", 65534, None, 1).await;
    assert!(res.is_err(), "probe should fail on unreachable port");
    let err = res.unwrap_err();
    match err {
        RockyError::TailscaleProbeTimeout {
            host,
            target,
            port,
            timeout,
            ..
        } => {
            assert_eq!(host, "edge");
            assert_eq!(target, "127.0.0.1");
            assert_eq!(port, 65534);
            assert_eq!(timeout, 1);
        }
        other => panic!("expected TailscaleProbeTimeout, got: {:?}", other),
    }
}

#[tokio::test]
async fn test_probe_rejects_invalid_host_name() {
    let res = probe_tailscale_connectivity("prod", "../../etc/passwd", "", "127.0.0.1", 22, None, 1).await;
    assert!(res.is_err());
    match res.unwrap_err() {
        RockyError::InvalidName { kind, name } => {
            assert_eq!(kind, Kind::Host);
            assert_eq!(name, "../../etc/passwd");
        }
        other => panic!("expected InvalidName, got: {:?}", other),
    }
}

#[tokio::test]
async fn test_probe_rejects_invalid_environment_name() {
    let res = probe_tailscale_connectivity("../../etc/passwd", "edge", "", "127.0.0.1", 22, None, 1).await;
    assert!(res.is_err());
    match res.unwrap_err() {
        RockyError::InvalidName { kind, name } => {
            assert_eq!(kind, Kind::Environment);
            assert_eq!(name, "../../etc/passwd");
        }
        other => panic!("expected InvalidName, got: {:?}", other),
    }
}

#[test]
fn test_probe_build_args_structure() {
    let args = build_probe_args(22, None, "192.168.1.50");
    assert!(args.contains(&"-p".to_string()));
    assert!(args.contains(&"22".to_string()));
    assert!(args.contains(&"UserKnownHostsFile=/dev/null".to_string()));
    assert!(args.contains(&"/dev/null".to_string()));
    assert!(args.contains(&"rocky@192.168.1.50".to_string()));
    assert!(args.contains(&"tailscale".to_string()));
    assert!(args.contains(&"status".to_string()));
    assert!(args.contains(&"--json".to_string()));
}

#[test]
fn test_probe_online_detection() {
    assert!(is_tailscale_online(r#"{"BackendState": "Running"}"#));
    assert!(is_tailscale_online(r#"{"BackendState":"Running"}"#));
    assert!(!is_tailscale_online(r#"{"BackendState": "Starting"}"#));
    assert!(!is_tailscale_online(r#"{"BackendState": "NeedsLogin"}"#));
    assert!(!is_tailscale_online(r#"{"BackendState": "Stopped"}"#));
    assert!(!is_tailscale_online("Logged out."));
    assert!(!is_tailscale_online("Tailscale is stopped."));
    assert!(!is_tailscale_online(""));
}
