use std::collections::HashMap;
use std::path::{Path, PathBuf};

use rnix::ast::{self, HasEntry};
use snafu::{OptionExt, ResultExt};
use tap::prelude::*;

use crate::error::{
    AlreadyExistsSnafu, AmbiguousEnvironmentSnafu, IoSnafu, Kind, NixParseSnafu, NotFoundSnafu,
    Result,
};
use crate::utils::validate::validate_name;

async fn verify_fleet(dir: &Path) -> Result<PathBuf> {
    let flake_path = dir.join("flake.nix");
    flake_path
        .is_file()
        .then_some(&flake_path)
        .context(NotFoundSnafu {
            kind: Kind::Fleet,
            name: "flake.nix".to_string(),
            path: dir.to_path_buf(),
        })?;

    let abs_path = std::fs::canonicalize(dir).unwrap_or_else(|_| dir.to_path_buf());
    let expr = format!(
        "(builtins.getFlake (toString \"{}\")).rockyFleet or false",
        abs_path.display()
    );

    super::eval::eval(&expr)
        .await
        .map(|out| out.trim() == "true")
        .unwrap_or(false)
        .then_some(abs_path)
        .context(NotFoundSnafu {
            kind: Kind::Fleet,
            name: "rocky".to_string(),
            path: dir.to_path_buf(),
        })
}

pub async fn with_fleet<F, Fut, T>(target: Option<&Path>, f: F) -> Result<T>
where
    F: FnOnce(PathBuf) -> Fut,
    Fut: std::future::Future<Output = Result<T>>,
{
    let target_path = target.map(|p| Ok(p.to_path_buf())).unwrap_or_else(|| {
        std::env::current_dir().context(IoSnafu {
            path: PathBuf::from("."),
        })
    })?;

    verify_fleet(&target_path)
        .await
        .tap_ok(|root| tracing::debug!(fleet_root = %root.display(), "verified fleet root"))?
        .pipe(f)
        .await
}

pub fn resolve_environment(fleet_root: &Path, env: Option<&str>) -> Result<(String, PathBuf)> {
    let envs_dir = fleet_root.join("environments");
    let env_name = match env {
        Some(e) => {
            validate_name(Kind::Environment, e)?;
            let target_env_dir = envs_dir.join(e);
            target_env_dir
                .is_dir()
                .then(|| e.to_string())
                .ok_or_else(|| {
                    NotFoundSnafu {
                        kind: Kind::Environment,
                        name: e.to_string(),
                        path: target_env_dir,
                    }
                    .build()
                })?
        }
        None => {
            if !envs_dir.is_dir() {
                return NotFoundSnafu {
                    kind: Kind::Environment,
                    name: "any".to_string(),
                    path: envs_dir,
                }
                .fail();
            }

            let available: Vec<String> = std::fs::read_dir(&envs_dir)
                .context(IoSnafu {
                    path: envs_dir.clone(),
                })?
                .filter_map(|entry| entry.ok())
                .filter(|entry| entry.path().is_dir())
                .filter_map(|entry| entry.file_name().into_string().ok())
                .filter(|name| {
                    !name.starts_with('.') && validate_name(Kind::Environment, name).is_ok()
                })
                .collect::<Vec<_>>()
                .tap_mut(|v| v.sort());

            match available.as_slice() {
                [single] => single.clone(),
                [] => {
                    return NotFoundSnafu {
                        kind: Kind::Environment,
                        name: "any".to_string(),
                        path: envs_dir,
                    }
                    .fail();
                }
                _ => {
                    return AmbiguousEnvironmentSnafu {
                        environments: available,
                    }
                    .fail();
                }
            }
        }
    };

    let env_dir = envs_dir.join(&env_name);
    Ok((env_name, env_dir))
}

pub fn resolve_secrets_dir(env_dir: &Path) -> PathBuf {
    let nested = env_dir.join("secrets");
    if nested.is_dir() {
        nested
    } else {
        env_dir.to_path_buf()
    }
}

use super::ast::{attr_to_name, parse_root_attr_set, unwrap_attr_set};

fn verify_syntax(updated: String, path: &Path) -> Result<String> {
    let parse = rnix::Root::parse(&updated);
    let errors = parse.errors();
    if !errors.is_empty() {
        return NixParseSnafu {
            path: path.to_path_buf(),
            reason: format!("resulted in invalid Nix syntax: {:?}", errors),
        }
        .fail();
    }
    Ok(updated)
}

pub fn insert_host_entry(content: &str, host_name: &str, path: &Path) -> Result<String> {
    validate_name(Kind::Host, host_name)?;

    let root_set = parse_root_attr_set(content, path)?;

    let mut hosts_attr_set: Option<ast::AttrSet> = None;
    let mut has_dotted_hosts = false;

    for entry in root_set.attrpath_values() {
        let Some(attrpath) = entry.attrpath() else {
            continue;
        };

        let mut attrs = attrpath.attrs();
        let Some(first_attr) = attrs.next() else {
            continue;
        };

        if attr_to_name(&first_attr).as_deref() != Some("hosts") {
            continue;
        }

        if let Some(second_attr) = attrs.next() {
            has_dotted_hosts = true;
            if attr_to_name(&second_attr).as_deref() == Some(host_name) {
                return AlreadyExistsSnafu {
                    kind: Kind::Host,
                    name: host_name.to_string(),
                    path: path.to_path_buf(),
                }
                .fail();
            }
            continue;
        }

        if let Some(val_expr) = entry.value() {
            if let Some(hosts_set) = unwrap_attr_set(val_expr) {
                for child in hosts_set.attrpath_values() {
                    if let Some(child_attrpath) = child.attrpath() {
                        if let Some(child_first) = child_attrpath.attrs().next() {
                            if attr_to_name(&child_first).as_deref() == Some(host_name) {
                                return AlreadyExistsSnafu {
                                    kind: Kind::Host,
                                    name: host_name.to_string(),
                                    path: path.to_path_buf(),
                                }
                                .fail();
                            }
                        }
                    }
                }
                hosts_attr_set = Some(hosts_set);
            }
        }
    }

    match hosts_attr_set {
        Some(hosts_set) => {
            let r_curly = hosts_set.r_curly_token().context(NixParseSnafu {
                path: path.to_path_buf(),
                reason: "missing closing brace for hosts attribute set",
            })?;
            let r_pos = usize::from(r_curly.text_range().start());

            let snippet = format!("  \"{host_name}\" = {{\n    services = [ ];\n  }};\n");

            let mut updated = content[..r_pos].to_string();
            if !updated.ends_with('\n') {
                updated.push('\n');
            }
            updated.push_str(&snippet);
            updated.push_str(&content[r_pos..]);
            verify_syntax(updated, path)
        }
        None => {
            let root_r_curly = root_set.r_curly_token().context(NixParseSnafu {
                path: path.to_path_buf(),
                reason: "missing closing brace for root attribute set",
            })?;
            let root_r_pos = usize::from(root_r_curly.text_range().start());
            let snippet = if has_dotted_hosts {
                format!("  hosts.\"{host_name}\" = {{\n    services = [ ];\n  }};\n")
            } else {
                format!(
                    "  hosts = {{\n    \"{host_name}\" = {{\n      services = [ ];\n    }};\n  }};\n"
                )
            };
            let mut updated = content[..root_r_pos].to_string();
            if !updated.ends_with('\n') {
                updated.push('\n');
            }
            updated.push_str(&snippet);
            updated.push_str(&content[root_r_pos..]);
            verify_syntax(updated, path)
        }
    }
}

pub async fn register_host(fleet_nix_path: &Path, host_name: &str) -> Result<()> {
    let content = crate::fs::read_file(fleet_nix_path)?;
    let updated = insert_host_entry(&content, host_name, fleet_nix_path)?;
    crate::fs::write_file(fleet_nix_path, &updated)?;
    super::fmt([fleet_nix_path]).await?;
    Ok(())
}

fn unwrap_parens(mut expr: ast::Expr) -> ast::Expr {
    while let ast::Expr::Paren(ref paren) = expr {
        if let Some(inner) = paren.expr() {
            expr = inner;
        } else {
            break;
        }
    }
    expr
}

fn extract_services_from_attr_set(attr_set: &ast::AttrSet, services: &mut Vec<String>) {
    for entry in attr_set.attrpath_values() {
        if let Some(attrpath) = entry.attrpath() {
            if let Some(first) = attrpath.attrs().next() {
                if let Some(name) = attr_to_name(&first) {
                    services.push(name);
                }
            }
        }
    }
}

fn extract_services_from_expr(expr: ast::Expr) -> Vec<String> {
    let current = unwrap_parens(expr);
    let mut services = Vec::new();
    match current {
        ast::Expr::List(list) => {
            for item in list.items() {
                match unwrap_parens(item) {
                    ast::Expr::Str(s) => {
                        let raw = s.to_string();
                        let clean = raw.trim_matches('\'').trim_matches('"').trim();
                        if !clean.is_empty() {
                            services.push(clean.to_string());
                        }
                    }
                    ast::Expr::Ident(ident) => {
                        let name = ident.to_string().trim().to_string();
                        if !name.is_empty() {
                            services.push(name);
                        }
                    }
                    ast::Expr::AttrSet(attr_set) => {
                        extract_services_from_attr_set(&attr_set, &mut services);
                    }
                    _ => {}
                }
            }
        }
        ast::Expr::AttrSet(attr_set) => {
            extract_services_from_attr_set(&attr_set, &mut services);
        }
        _ => {}
    }
    services
}

fn extract_services_from_host_set(host_set: &ast::AttrSet) -> Vec<String> {
    let mut services = Vec::new();
    for entry in host_set.attrpath_values() {
        let Some(attrpath) = entry.attrpath() else {
            continue;
        };
        let mut attrs = attrpath.attrs();
        let Some(first_attr) = attrs.next() else {
            continue;
        };
        if attr_to_name(&first_attr).as_deref() != Some("services") {
            continue;
        }

        if let Some(second_attr) = attrs.next() {
            if let Some(name) = attr_to_name(&second_attr) {
                services.push(name);
            }
            continue;
        }

        if let Some(val_expr) = entry.value() {
            services.extend(extract_services_from_expr(val_expr));
        }
    }
    services
}

pub fn extract_host_services(fleet_src: &str, path: &Path) -> Result<HashMap<String, Vec<String>>> {
    let root_set = parse_root_attr_set(fleet_src, path)?;
    let mut map: HashMap<String, Vec<String>> = HashMap::new();

    for entry in root_set.attrpath_values() {
        let Some(attrpath) = entry.attrpath() else {
            continue;
        };

        let mut attrs = attrpath.attrs();
        let Some(first_attr) = attrs.next() else {
            continue;
        };

        if attr_to_name(&first_attr).as_deref() != Some("hosts") {
            continue;
        }

        if let Some(second_attr) = attrs.next() {
            let Some(host_name) = attr_to_name(&second_attr) else {
                continue;
            };

            let host_services = map.entry(host_name.clone()).or_default();

            if let Some(third_attr) = attrs.next() {
                if attr_to_name(&third_attr).as_deref() == Some("services") {
                    if let Some(fourth_attr) = attrs.next() {
                        if let Some(svc_name) = attr_to_name(&fourth_attr) {
                            host_services.push(svc_name);
                        }
                    } else if let Some(val_expr) = entry.value() {
                        host_services.extend(extract_services_from_expr(val_expr));
                    }
                }
            } else if let Some(val_expr) = entry.value() {
                if let Some(host_set) = unwrap_attr_set(val_expr) {
                    host_services.extend(extract_services_from_host_set(&host_set));
                }
            }
            continue;
        }

        let Some(val_expr) = entry.value() else {
            continue;
        };

        if let Some(hosts_set) = unwrap_attr_set(val_expr) {
            for inner_entry in hosts_set.attrpath_values() {
                let Some(inner_path) = inner_entry.attrpath() else {
                    continue;
                };
                let mut inner_attrs = inner_path.attrs();
                let Some(host_attr) = inner_attrs.next() else {
                    continue;
                };
                let Some(host_name) = attr_to_name(&host_attr) else {
                    continue;
                };

                let host_services = map.entry(host_name.clone()).or_default();

                if let Some(second_inner) = inner_attrs.next() {
                    if attr_to_name(&second_inner).as_deref() == Some("services") {
                        if let Some(third_inner) = inner_attrs.next() {
                            if let Some(svc_name) = attr_to_name(&third_inner) {
                                host_services.push(svc_name);
                            }
                        } else if let Some(inner_val) = inner_entry.value() {
                            host_services.extend(extract_services_from_expr(inner_val));
                        }
                    }
                } else if let Some(inner_val) = inner_entry.value() {
                    if let Some(host_set) = unwrap_attr_set(inner_val) {
                        host_services.extend(extract_services_from_host_set(&host_set));
                    }
                }
            }
        }
    }

    for services in map.values_mut() {
        let mut seen = std::collections::HashSet::new();
        services.retain(|s| seen.insert(s.clone()));
    }

    Ok(map)
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::tempdir;

    #[tokio::test]
    async fn test_verify_fleet_success() {
        let dir = tempdir().unwrap();
        let flake = r#"{
  description = "rocky fleet";
  outputs = inputs: {
    rockyFleet = true;
  };
}
"#;
        std::fs::write(dir.path().join("flake.nix"), flake).unwrap();

        let verified = verify_fleet(dir.path()).await;
        assert!(verified.is_ok());
        assert_eq!(verified.unwrap(), dir.path().canonicalize().unwrap());
    }

    #[tokio::test]
    async fn test_verify_fleet_missing_flake() {
        let dir = tempdir().unwrap();
        let res = verify_fleet(dir.path()).await;
        match res {
            Err(crate::error::RockyError::NotFound { kind, name, .. }) => {
                assert_eq!(kind, Kind::Fleet);
                assert_eq!(name, "flake.nix");
            }
            other => panic!("expected NotFound flake.nix, got {:?}", other),
        }
    }

    #[tokio::test]
    async fn test_verify_fleet_not_a_rocky_fleet() {
        let dir = tempdir().unwrap();
        let foreign_flake = r#"{
  description = "some other rust project";
  outputs = inputs: {};
}
"#;
        std::fs::write(dir.path().join("flake.nix"), foreign_flake).unwrap();

        let res = verify_fleet(dir.path()).await;
        match res {
            Err(crate::error::RockyError::NotFound { kind, name, .. }) => {
                assert_eq!(kind, Kind::Fleet);
                assert_eq!(name, "rocky");
            }
            other => panic!("expected NotFound rocky, got {:?}", other),
        }
    }

    #[tokio::test]
    async fn test_with_fleet_some_executes_closure_with_fleet_root() {
        let dir = tempdir().unwrap();
        let flake = r#"{
  outputs = _: {
    rockyFleet = true;
  };
}
"#;
        std::fs::write(dir.path().join("flake.nix"), flake).unwrap();

        let expected = dir.path().canonicalize().unwrap();
        let executed = with_fleet(Some(dir.path()), |root| async move {
            assert_eq!(root, expected);
            Ok(42)
        })
        .await;

        assert_eq!(executed.unwrap(), 42);
    }

    #[tokio::test]
    async fn test_with_fleet_some_rejects_non_fleet() {
        let dir = tempdir().unwrap();
        let res = with_fleet(Some(dir.path()), |_| async { Ok(()) }).await;
        match res {
            Err(crate::error::RockyError::NotFound { kind, .. }) => {
                assert_eq!(kind, Kind::Fleet);
            }
            other => panic!("expected NotFound Fleet, got {:?}", other),
        }
    }

    #[tokio::test]
    async fn test_with_fleet_none_checks_current_working_directory() {
        // Current directory is the rocky repo root, which is not a deployed fleet (lacks rockyFleet = true)
        let res = with_fleet(None, |_| async { Ok(()) }).await;
        match res {
            Err(crate::error::RockyError::NotFound { kind, name, .. }) => {
                assert_eq!(kind, Kind::Fleet);
                assert_eq!(name, "rocky");
            }
            other => panic!("expected NotFound rocky fleet, got {:?}", other),
        }
    }

    #[tokio::test]
    async fn test_with_fleet_propagates_closure_error() {
        let dir = tempdir().unwrap();
        let flake = r#"{
  outputs = _: {
    rockyFleet = true;
  };
}
"#;
        std::fs::write(dir.path().join("flake.nix"), flake).unwrap();

        let res = with_fleet(Some(dir.path()), |root| async move {
            crate::error::NotFoundSnafu {
                kind: Kind::Host,
                name: "dummy-host".to_string(),
                path: root,
            }
            .fail::<()>()
        })
        .await;

        match res {
            Err(crate::error::RockyError::NotFound { kind, name, .. }) => {
                assert_eq!(kind, Kind::Host);
                assert_eq!(name, "dummy-host");
            }
            other => panic!("expected NotFound Host from closure, got {:?}", other),
        }
    }

    #[test]
    fn test_insert_host_entry_empty_hosts() {
        let input = r#"{
  settings = {
    domain = "prod.rocky.internal";
  };
  hosts = { };
}
"#;
        let res = insert_host_entry(input, "web-01", Path::new("fleet.nix")).unwrap();
        assert!(res.contains(r#""web-01" = {"#));
        assert!(res.contains(r#"services = [ ];"#));

        // Re-parsing the output must be valid Nix
        let parse = rnix::Root::parse(&res);
        assert!(parse.errors().is_empty());
    }

    #[test]
    fn test_insert_host_entry_existing_hosts() {
        let input = r#"{
  hosts = {
    "001" = {
      services = [ ];
    };
  };
}
"#;
        let res = insert_host_entry(input, "002", Path::new("fleet.nix")).unwrap();
        assert!(res.contains(r#""001" = {"#));
        assert!(res.contains(r#""002" = {"#));

        let parse = rnix::Root::parse(&res);
        assert!(parse.errors().is_empty());
    }

    #[test]
    fn test_insert_host_entry_duplicate_rejected() {
        let input = r#"{
  hosts = {
    "web-01" = {
      services = [ ];
    };
  };
}
"#;
        let res = insert_host_entry(input, "web-01", Path::new("fleet.nix"));
        match res {
            Err(crate::error::RockyError::AlreadyExists { kind, name, .. }) => {
                assert_eq!(kind, Kind::Host);
                assert_eq!(name, "web-01");
            }
            other => panic!("expected AlreadyExists Host, got {:?}", other),
        }
    }

    #[test]
    fn test_insert_host_entry_without_hosts_set() {
        let input = r#"{
  settings = {
    domain = "staging.rocky.internal";
  };
}
"#;
        let res = insert_host_entry(input, "edge-01", Path::new("fleet.nix")).unwrap();
        assert!(res.contains("hosts = {"));
        assert!(res.contains(r#""edge-01" = {"#));

        let parse = rnix::Root::parse(&res);
        assert!(parse.errors().is_empty());
    }

    #[test]
    fn test_insert_host_entry_syntax_error() {
        let input = "not valid nix {";
        let res = insert_host_entry(input, "web-01", Path::new("fleet.nix"));
        match res {
            Err(crate::error::RockyError::NixParse { .. }) => {}
            other => panic!("expected NixParse, got {:?}", other),
        }
    }

    #[test]
    fn test_insert_host_entry_dotted_syntax() {
        let input = r#"{
  hosts."001" = {
    services = [ ];
  };
}
"#;
        let res = insert_host_entry(input, "002", Path::new("fleet.nix")).unwrap();
        assert!(res.contains(r#"hosts."002" = {"#));

        let parse = rnix::Root::parse(&res);
        assert!(parse.errors().is_empty());
    }

    #[test]
    fn test_insert_host_entry_preserves_comments() {
        let input = r#"{
  hosts = {
    # Keep this comment intact
  };
}
"#;
        let res = insert_host_entry(input, "web-01", Path::new("fleet.nix")).unwrap();
        assert!(res.contains("# Keep this comment intact"));
        assert!(res.contains(r#""web-01" = {"#));

        let parse = rnix::Root::parse(&res);
        assert!(parse.errors().is_empty());
    }

    #[test]
    fn test_insert_host_entry_rejects_invalid_name() {
        let input = "{\n  hosts = {};\n}\n";
        let res = insert_host_entry(input, "../evil", Path::new("fleet.nix"));
        assert!(res.is_err());
    }

    #[test]
    fn test_insert_host_entry_preserves_dotted_syntax_comments() {
        let input = r#"# Dotted fleet definition
{
  # Host 001 definition
  hosts."001" = {
    services = [ "caddy" ];
  };
  # End of hosts list
}
"#;
        let res = insert_host_entry(input, "002", Path::new("fleet.nix")).unwrap();
        assert!(res.contains("# Dotted fleet definition"));
        assert!(res.contains("# Host 001 definition"));
        assert!(res.contains("# End of hosts list"));
        assert!(res.contains(r#"hosts."001" = {"#));
        assert!(res.contains(r#"hosts."002" = {"#));

        let parse = rnix::Root::parse(&res);
        assert!(parse.errors().is_empty());
    }

    #[tokio::test]
    async fn test_register_host_preserves_comments_and_custom_fields() {
        let dir = tempdir().unwrap();
        let fleet_path = dir.path().join("fleet.nix");
        let initial_content = r#"# Fleet configuration header
{
  # General fleet settings
  settings = {
    domain = "prod.rocky.internal";
  };

  # Host declarations
  hosts = {
    # Primary database host
    "db-01" = {
      services = [ "postgres" ]; # relational storage
    };
    # Trailing comment inside hosts set
  };

  # Custom metadata section
  meta = {
    team = "platform";
  };
}
# End of file comment
"#;
        std::fs::write(&fleet_path, initial_content).unwrap();

        let res = register_host(&fleet_path, "api-01").await;
        assert!(res.is_ok(), "register_host failed: {:?}", res);

        let updated = std::fs::read_to_string(&fleet_path).unwrap();

        // Verify all comments are preserved
        assert!(updated.contains("# Fleet configuration header"));
        assert!(updated.contains("# General fleet settings"));
        assert!(updated.contains("# Host declarations"));
        assert!(updated.contains("# Primary database host"));
        assert!(updated.contains("# relational storage"));
        assert!(updated.contains("# Trailing comment inside hosts set"));
        assert!(updated.contains("# Custom metadata section"));
        assert!(updated.contains("# End of file comment"));

        // Verify custom attributes and both hosts are present
        assert!(updated.contains("prod.rocky.internal"));
        assert!(updated.contains("platform"));
        assert!(updated.contains(r#""db-01""#));
        assert!(updated.contains(r#""api-01""#));

        // Output must parse cleanly as valid Nix
        let parse = rnix::Root::parse(&updated);
        assert!(parse.errors().is_empty());
    }

    #[test]
    fn test_extract_host_services_various_formats() {
        let fleet = r#"{
  hosts = {
    node-01 = {
      services = [ "caddy" "api" ];
    };
    node-02 = {
      services = {
        postgres = {};
        redis = {};
      };
    };
    node-03 = {
      services = [ ];
    };
    node-04 = {
      services.monitoring = {};
    };
    node-05 = {};
  };
  hosts."node-06" = {
    services = [ "worker" "caddy" "worker" ];
  };
  hosts."node-07".services = [ "gateway" ];
  hosts."node-08".services = [
    {
      api = {
        replicas = 3;
        basePort = 8000;
      };
    }
  ];
}
"#;
        let map = extract_host_services(fleet, Path::new("fleet.nix")).unwrap();
        assert_eq!(map.get("node-01").unwrap(), &vec!["caddy", "api"]);
        assert_eq!(map.get("node-02").unwrap(), &vec!["postgres", "redis"]);
        assert!(map.get("node-03").unwrap().is_empty());
        assert_eq!(map.get("node-04").unwrap(), &vec!["monitoring"]);
        assert!(map.get("node-05").unwrap().is_empty());
        // Deduplicated preserving order
        assert_eq!(map.get("node-06").unwrap(), &vec!["worker", "caddy"]);
        assert_eq!(map.get("node-07").unwrap(), &vec!["gateway"]);
        assert_eq!(map.get("node-08").unwrap(), &vec!["api"]);
    }

    #[test]
    fn test_extract_host_services_syntax_error() {
        let fleet = "{ hosts = { bad = ; }; }";
        let res = extract_host_services(fleet, Path::new("fleet.nix"));
        assert!(res.is_err());
    }

    #[test]
    fn test_extract_host_services_replicated_edge_cases() {
        let fleet = r#"{
  hosts = {
    edge = {
      services = [
        "caddy"
        {
          api = {
            replicas = 3;
            basePort = 8000;
          };
          worker = {
            replicas = 2;
          };
        }
        "caddy"
        {
          api = {};
        }
      ];
    };
    worker = {
      services = [
        { "auth-service" = { replicas = 1; }; }
        { "storage-api" = { replicas = 2; }; }
      ];
    };
  };
}
"#;
        let map = extract_host_services(fleet, Path::new("fleet.nix")).unwrap();
        // Should preserve order and deduplicate: "caddy", "api", "worker"
        assert_eq!(map.get("edge").unwrap(), &vec!["caddy", "api", "worker"]);
        assert_eq!(
            map.get("worker").unwrap(),
            &vec!["auth-service", "storage-api"]
        );
    }

    #[test]
    fn test_extract_host_services_parenthesized_and_attrset_forms() {
        let fleet = r#"{
  hosts = {
    node-paren = {
      services = [
        ("caddy")
        ({
          api = {
            replicas = 2;
          };
        })
      ];
    };
    node-attrset = {
      services = {
        web = { replicas = 1; };
        db = {};
      };
    };
    node-paren-top = {
      services = ([ "metrics" ]);
    };
  };
}
"#;
        let map = extract_host_services(fleet, Path::new("fleet.nix")).unwrap();
        assert_eq!(map.get("node-paren").unwrap(), &vec!["caddy", "api"]);
        assert_eq!(map.get("node-attrset").unwrap(), &vec!["web", "db"]);
        assert_eq!(map.get("node-paren-top").unwrap(), &vec!["metrics"]);
    }

    #[test]
    fn test_resolve_environment_ignores_hidden_and_invalid_directories() {
        let dir = tempfile::tempdir().unwrap();
        let fleet_root = dir.path();
        let envs_dir = fleet_root.join("environments");
        std::fs::create_dir_all(envs_dir.join(".direnv")).unwrap();
        std::fs::create_dir_all(envs_dir.join(".git")).unwrap();
        std::fs::create_dir_all(envs_dir.join("prod")).unwrap();

        let (name, path) = resolve_environment(fleet_root, None).unwrap();
        assert_eq!(name, "prod");
        assert_eq!(path, envs_dir.join("prod"));
    }
}
