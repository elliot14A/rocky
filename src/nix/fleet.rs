use std::path::{Path, PathBuf};

use rnix::ast::{self, HasEntry};
use snafu::{OptionExt, ResultExt};
use tap::prelude::*;

use crate::error::{AlreadyExistsSnafu, IoSnafu, Kind, NixParseSnafu, NotFoundSnafu, Result};

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

use super::ast::{attr_to_name, parse_root_attr_set};

pub fn insert_host_entry(content: &str, host_name: &str, path: &Path) -> Result<String> {
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

        if let Some(ast::Expr::AttrSet(hosts_set)) = entry.value() {
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
            Ok(updated)
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
            Ok(updated)
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
}
