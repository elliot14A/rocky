use std::path::{Path, PathBuf};

use rnix::ast::HasEntry;
use snafu::ensure;
use tap::prelude::*;

use crate::error::{Kind, MeshSnafu, NotFoundSnafu, Result};
use crate::templates::service::ServiceKind;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ResolvedService {
    pub name: String,
    pub kind: ServiceKind,
    pub journal_id: String,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ResolvedLogTarget {
    pub host: String,
    pub service: Option<ResolvedService>,
}

#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct JournalOptions {
    pub follow: bool,
    pub tail: Option<usize>,
    pub since: Option<String>,
    pub until: Option<String>,
    pub timestamps: bool,
}

pub fn detect_service_kind(
    service_name: &str,
    src: &str,
    path: &Path,
) -> Result<(ServiceKind, String)> {
    let root_set = crate::nix::parse_root_attr_set(src, path)?;

    let mut podman_match = false;
    let mut systemd_unit: Option<String> = None;
    let mut nixpkgs_module: Option<String> = None;

    scan_attr_set_for_service(
        &root_set,
        &[],
        &mut podman_match,
        &mut systemd_unit,
        &mut nixpkgs_module,
    );

    if podman_match {
        return Ok((ServiceKind::Podman, format!("podman-{service_name}")));
    }

    if let Some(unit) = systemd_unit {
        return Ok((ServiceKind::Systemd, unit));
    }

    if let Some(module) = nixpkgs_module {
        return Ok((ServiceKind::Systemd, module));
    }

    Ok((ServiceKind::Systemd, service_name.to_string()))
}

fn scan_attr_set_for_service(
    attr_set: &rnix::ast::AttrSet,
    prefix: &[String],
    podman_match: &mut bool,
    systemd_unit: &mut Option<String>,
    nixpkgs_module: &mut Option<String>,
) {
    if *podman_match {
        return;
    }

    for entry in attr_set.attrpath_values() {
        let Some(attrpath) = entry.attrpath() else {
            continue;
        };

        let current_parts: Vec<String> = attrpath
            .attrs()
            .filter_map(|a| crate::nix::attr_to_name(&a))
            .collect();

        let mut full_path = prefix.to_vec();
        full_path.extend(current_parts);

        let first = full_path.first().map(|s| s.as_str());
        let second = full_path.get(1).map(|s| s.as_str());
        let third = full_path.get(2).map(|s| s.as_str());

        match (first, second) {
            (Some("virtualisation"), Some("oci-containers")) => {
                *podman_match = true;
                return;
            }
            (Some("systemd"), Some("services")) => {
                if let (Some(unit), None) = (third, &systemd_unit) {
                    *systemd_unit = Some(unit.to_string());
                }
            }
            (Some("services"), Some(module)) if nixpkgs_module.is_none() => {
                *nixpkgs_module = Some(module.to_string());
            }
            _ => {}
        }

        if let Some(val_expr) = entry.value() {
            if let Some(inner_set) = crate::nix::unwrap_attr_set(val_expr) {
                scan_attr_set_for_service(
                    &inner_set,
                    &full_path,
                    podman_match,
                    systemd_unit,
                    nixpkgs_module,
                );
            }
        }
    }
}

pub fn validate_service_exists(fleet_root: &Path, service_name: &str) -> Result<PathBuf> {
    let clean = service_name.trim().trim_end_matches(".service");
    crate::validate::validate_name(Kind::Service, clean)?;
    let service_nix = fleet_root.join("services").join(format!("{clean}.nix"));

    ensure!(
        service_nix.is_file(),
        NotFoundSnafu {
            kind: Kind::Service,
            name: clean.to_string(),
            path: service_nix,
        }
    );

    Ok(service_nix)
}

pub fn resolve_service_target(
    fleet_root: &Path,
    env: &str,
    target_or_service: Option<&str>,
    explicit_host: Option<&str>,
) -> Result<ResolvedLogTarget> {
    crate::validate::validate_name(Kind::Environment, env)?;

    let env_dir = fleet_root.join("environments").join(env);
    let fleet_nix = env_dir.join("fleet.nix");

    ensure!(
        fleet_nix.is_file(),
        NotFoundSnafu {
            kind: Kind::Environment,
            name: env.to_string(),
            path: env_dir,
        }
    );

    let content = crate::fs::read_file(&fleet_nix)?;
    let host_map = crate::nix::extract_host_services(&content, &fleet_nix)?;

    ensure!(
        !host_map.is_empty(),
        NotFoundSnafu {
            kind: Kind::Host,
            name: format!("(no hosts in {env})"),
            path: fleet_nix,
        }
    );

    if let Some(host) = explicit_host {
        crate::validate::validate_name(Kind::Host, host)?;

        ensure!(
            host_map.contains_key(host),
            NotFoundSnafu {
                kind: Kind::Host,
                name: host.to_string(),
                path: fleet_nix,
            }
        );

        let Some(raw_target) = target_or_service else {
            return Ok(ResolvedLogTarget {
                host: host.to_string(),
                service: None,
            });
        };

        let clean = raw_target.trim().trim_end_matches(".service");
        if let Some(host_services) = host_map.get(host) {
            let is_assigned = host_services
                .iter()
                .any(|s| s.trim().trim_end_matches(".service") == clean);
            ensure!(
                is_assigned,
                NotFoundSnafu {
                    kind: Kind::Service,
                    name: clean.to_string(),
                    path: fleet_nix,
                }
            );
        }

        let service_nix = validate_service_exists(fleet_root, clean)?;
        let service_src = crate::fs::read_file(&service_nix)?;
        let (kind, journal_id) = detect_service_kind(clean, &service_src, &service_nix)?;

        return Ok(ResolvedLogTarget {
            host: host.to_string(),
            service: Some(ResolvedService {
                name: clean.to_string(),
                kind,
                journal_id,
            }),
        });
    }

    let Some(raw_target) = target_or_service else {
        let mut sorted_hosts: Vec<_> = host_map.keys().cloned().collect();
        sorted_hosts.sort();
        if sorted_hosts.len() > 1 {
            return crate::error::TargetParseSnafu {
                raw: env.to_string(),
                reason: format!(
                    "environment '{env}' contains multiple hosts ([{}]); specify target host with --host <name>",
                    sorted_hosts.join(", ")
                ),
            }
            .fail();
        }
        let host = sorted_hosts.into_iter().next().unwrap_or_default();
        return Ok(ResolvedLogTarget {
            host,
            service: None,
        });
    };

    let clean = raw_target.trim().trim_end_matches(".service");

    let mut matching_hosts: Vec<String> = host_map
        .iter()
        .filter(|(_, services)| {
            services
                .iter()
                .any(|s| s.trim().trim_end_matches(".service") == clean)
        })
        .map(|(host, _)| host.clone())
        .collect();
    matching_hosts.sort();

    if matching_hosts.len() > 1 {
        return crate::error::TargetParseSnafu {
            raw: clean.to_string(),
            reason: format!(
                "service '{clean}' is deployed on multiple hosts ([{}]); specify target host with --host <name>",
                matching_hosts.join(", ")
            ),
        }
        .fail();
    }

    let host = match matching_hosts.into_iter().next() {
        Some(h) => h,
        None => {
            if host_map.contains_key(clean) {
                crate::validate::validate_name(Kind::Host, clean)?;
                return Ok(ResolvedLogTarget {
                    host: clean.to_string(),
                    service: None,
                });
            }
            return NotFoundSnafu {
                kind: Kind::Service,
                name: raw_target.to_string(),
                path: fleet_nix,
            }
            .fail();
        }
    };

    let service_nix = validate_service_exists(fleet_root, clean)?;
    let service_src = crate::fs::read_file(&service_nix)?;
    let (kind, journal_id) = detect_service_kind(clean, &service_src, &service_nix)?;

    Ok(ResolvedLogTarget {
        host,
        service: Some(ResolvedService {
            name: clean.to_string(),
            kind,
            journal_id,
        }),
    })
    .tap_ok(|target| {
        tracing::debug!(
            host = %target.host,
            service = ?target.service,
            "resolved service target"
        )
    })
}

pub fn validate_timestamp(param: &str, value: &str) -> Result<()> {
    ensure!(
        !value.chars().any(|c| c.is_control()),
        crate::error::TargetParseSnafu {
            raw: value.to_string(),
            reason: format!("timestamp option --{param} contains invalid control characters"),
        }
    );
    Ok(())
}

pub async fn execute_logs(
    fleet_root: &Path,
    env_raw: &str,
    service_raw: Option<&str>,
    explicit_host: Option<&str>,
    identity_arg: Option<&Path>,
    opts: &JournalOptions,
) -> Result<()> {
    if let Some(since) = &opts.since {
        validate_timestamp("since", since)?;
    }
    if let Some(until) = &opts.until {
        validate_timestamp("until", until)?;
    }

    crate::validate::validate_name(Kind::Environment, env_raw)?;
    let env_dir = fleet_root.join("environments").join(env_raw);
    ensure!(
        env_dir.is_dir(),
        NotFoundSnafu {
            kind: Kind::Environment,
            name: env_raw.to_string(),
            path: env_dir,
        }
    );

    let target = resolve_service_target(fleet_root, env_raw, service_raw, explicit_host)?;

    let domain = crate::process::mesh::resolve_domain(fleet_root, env_raw).await?;
    let (is_joined, _) = crate::process::mesh::check_workstation_tailscale(&domain, env_raw).await?;
    if !is_joined {
        return MeshSnafu {
            reason: format!(
                "Workstation is not connected to fleet mesh '{env_raw}'. Run 'rocky mesh join {env_raw}' first."
            ),
        }
        .fail();
    }

    let mesh_node = format!("{env_raw}-{}", target.host);
    let resolved_addr = crate::process::ssh::resolve_target_address(&mesh_node, &domain).await;
    let identity = crate::process::ssh::resolve_identity_key(identity_arg)?;

    let journal_args = build_journalctl_args(&target, opts);
    let remote_cmd = shlex::try_join(journal_args.iter().map(|s| s.as_str())).map_err(|_| {
        crate::error::TargetParseSnafu {
            raw: target.host.clone(),
            reason: "journal argument contains invalid nul byte".to_string(),
        }
        .build()
    })?;

    let ssh_args = crate::process::ssh::build_ssh_args(
        identity.as_deref(),
        &resolved_addr,
        crate::process::ssh::ROCKY_USER,
        &[remote_cmd],
    );

    crate::process::ssh::exec_ssh(&ssh_args)
}

pub fn build_journalctl_args(target: &ResolvedLogTarget, opts: &JournalOptions) -> Vec<String> {
    let mut args = vec![
        "journalctl".to_string(),
        "--no-pager".to_string(),
        "-q".to_string(),
    ];

    if let Some(service) = &target.service {
        args.push("-u".to_string());
        args.push(format!("{}.service", service.journal_id));
        args.push("-u".to_string());
        args.push(format!("{}-*.service", service.journal_id));
    }

    if opts.follow {
        args.push("-f".to_string());
    }

    if let Some(lines) = opts.tail {
        args.push("-n".to_string());
        args.push(lines.to_string());
    }

    if opts.timestamps {
        args.push("-o".to_string());
        args.push("short-iso".to_string());
    }

    if let Some(since) = &opts.since {
        if !since.trim().is_empty() {
            args.push("--since".to_string());
            args.push(since.trim().to_string());
        }
    }

    if let Some(until) = &opts.until {
        if !until.trim().is_empty() {
            args.push("--until".to_string());
            args.push(until.trim().to_string());
        }
    }

    args
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::tempdir;

    #[test]
    fn test_detect_service_kind_podman() {
        let src = r#"
        { replicateContainers, ... }:
        {
          virtualisation.oci-containers.containers = replicateContainers {
            service = "gotrue";
          };
        }
        "#;
        let (kind, journal_id) =
            detect_service_kind("gotrue", src, Path::new("gotrue.nix")).unwrap();
        assert_eq!(kind, ServiceKind::Podman);
        assert_eq!(journal_id, "podman-gotrue");
    }

    #[test]
    fn test_detect_service_kind_nested_attrset_podman() {
        let src = r#"
        { ... }:
        {
          virtualisation = {
            oci-containers = {
              containers = {
                app = {
                  image = "my-app:latest";
                };
              };
            };
          };
        }
        "#;
        let (kind, journal_id) =
            detect_service_kind("app", src, Path::new("app.nix")).unwrap();
        assert_eq!(kind, ServiceKind::Podman);
        assert_eq!(journal_id, "podman-app");
    }

    #[test]
    fn test_detect_service_kind_systemd_nixpkgs_module() {
        let src = r#"
        { config, ... }:
        {
          services.postgresql = {
            enable = true;
          };
        }
        "#;
        let (kind, journal_id) =
            detect_service_kind("postgres", src, Path::new("postgres.nix")).unwrap();
        assert_eq!(kind, ServiceKind::Systemd);
        assert_eq!(journal_id, "postgresql");
    }

    #[test]
    fn test_detect_service_kind_systemd_custom_unit() {
        let src = r#"
        { pkgs, ... }:
        {
          systemd.services.rustic-backup = {
            description = "Automated backup";
          };
        }
        "#;
        let (kind, journal_id) =
            detect_service_kind("rustic", src, Path::new("rustic.nix")).unwrap();
        assert_eq!(kind, ServiceKind::Systemd);
        assert_eq!(journal_id, "rustic-backup");
    }

    #[test]
    fn test_detect_service_kind_systemd_replicated_service() {
        let src = r#"
        { replicateService, ... }:
        {
          systemd.services = replicateService {
            service = "postgrest";
          };
        }
        "#;
        let (kind, journal_id) =
            detect_service_kind("postgrest", src, Path::new("postgrest.nix")).unwrap();
        assert_eq!(kind, ServiceKind::Systemd);
        assert_eq!(journal_id, "postgrest");
    }

    #[test]
    fn test_resolve_service_target_podman_and_systemd() {
        let dir = tempdir().unwrap();
        let fleet_root = dir.path();

        let env_dir = fleet_root.join("environments").join("prod");
        std::fs::create_dir_all(&env_dir).unwrap();

        let services_dir = fleet_root.join("services");
        std::fs::create_dir_all(&services_dir).unwrap();

        let fleet_nix = r#"{
            hosts = {
                edge = {
                    services = [ "caddy" ];
                };
                app = {
                    services = [ "gotrue" ];
                };
            };
        }"#;
        std::fs::write(env_dir.join("fleet.nix"), fleet_nix).unwrap();

        let caddy_nix = r#"{ services.caddy = { enable = true; }; }"#;
        std::fs::write(services_dir.join("caddy.nix"), caddy_nix).unwrap();

        let gotrue_nix = r#"{ virtualisation.oci-containers.containers.gotrue = {}; }"#;
        std::fs::write(services_dir.join("gotrue.nix"), gotrue_nix).unwrap();

        // Resolving caddy (systemd)
        let target = resolve_service_target(fleet_root, "prod", Some("caddy"), None).unwrap();
        assert_eq!(target.host, "edge");
        let svc = target.service.unwrap();
        assert_eq!(svc.name, "caddy");
        assert_eq!(svc.kind, ServiceKind::Systemd);
        assert_eq!(svc.journal_id, "caddy");

        // Resolving gotrue (podman)
        let target = resolve_service_target(fleet_root, "prod", Some("gotrue"), None).unwrap();
        assert_eq!(target.host, "app");
        let svc = target.service.unwrap();
        assert_eq!(svc.name, "gotrue");
        assert_eq!(svc.kind, ServiceKind::Podman);
        assert_eq!(svc.journal_id, "podman-gotrue");

        // Explicit host with service
        let target =
            resolve_service_target(fleet_root, "prod", Some("caddy"), Some("edge")).unwrap();
        assert_eq!(target.host, "edge");
        assert_eq!(target.service.unwrap().name, "caddy");

        // Explicit host without service returns host journal
        let target = resolve_service_target(fleet_root, "prod", None, Some("edge")).unwrap();
        assert_eq!(target.host, "edge");
        assert!(target.service.is_none());

        // Host name passed as target_or_service returns host journal
        let target = resolve_service_target(fleet_root, "prod", Some("app"), None).unwrap();
        assert_eq!(target.host, "app");
        assert!(target.service.is_none());

        // Mismatched host and service returns NotFound
        let err =
            resolve_service_target(fleet_root, "prod", Some("gotrue"), Some("edge")).unwrap_err();
        match err {
            crate::error::RockyError::NotFound { kind, name, .. } => {
                assert_eq!(kind, Kind::Service);
                assert_eq!(name, "gotrue");
            }
            other => panic!("expected NotFound, got {other:?}"),
        }

        // Missing service returns NotFound
        let err =
            resolve_service_target(fleet_root, "prod", Some("nonexistent"), None).unwrap_err();
        match err {
            crate::error::RockyError::NotFound { kind, name, .. } => {
                assert_eq!(kind, Kind::Service);
                assert_eq!(name, "nonexistent");
            }
            other => panic!("expected NotFound, got {other:?}"),
        }
    }

    #[test]
    fn test_build_journalctl_args_default_host_logs() {
        let opts = JournalOptions::default();
        let target = ResolvedLogTarget {
            host: "edge".to_string(),
            service: None,
        };
        let args = build_journalctl_args(&target, &opts);
        assert_eq!(args, vec!["journalctl", "--no-pager", "-q"]);
    }

    #[test]
    fn test_build_journalctl_args_service_with_follow_and_tail() {
        let opts = JournalOptions {
            follow: true,
            tail: Some(50),
            ..Default::default()
        };
        let target = ResolvedLogTarget {
            host: "app".to_string(),
            service: Some(ResolvedService {
                name: "gotrue".to_string(),
                kind: ServiceKind::Podman,
                journal_id: "podman-gotrue".to_string(),
            }),
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
                "50"
            ]
        );
    }
}
