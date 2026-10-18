use std::collections::BTreeSet;
use std::path::Path;

use rnix::ast::{self, HasEntry};
use tap::prelude::*;

use crate::error::Result;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct HostDiff {
    pub declared: Vec<String>,
    pub pending_install: Vec<String>,
    pub installed: Vec<String>,
    pub removed: Vec<String>,
}

use super::ast::{attr_to_name, parse_root_attr_set, unwrap_attr_set};

fn extract_fleet_hosts(fleet_src: &str) -> Result<BTreeSet<String>> {
    super::fleet::extract_host_services(fleet_src, Path::new("fleet.nix"))
        .map(|map| map.into_keys().collect())
}

fn extract_key_hosts(keys_src: &str) -> Result<BTreeSet<String>> {
    let root_set = parse_root_attr_set(keys_src, Path::new("keys.nix"))?;

    let mut hosts = BTreeSet::new();

    for entry in root_set.attrpath_values() {
        let Some(attrpath) = entry.attrpath() else {
            continue;
        };

        let mut attrs = attrpath.attrs();
        let Some(first_attr) = attrs.next() else {
            continue;
        };

        let Some(first_name) = attr_to_name(&first_attr) else {
            continue;
        };

        if first_name == "hostKeys" {
            if let Some(second_attr) = attrs.next() {
                if let Some(name) = attr_to_name(&second_attr) {
                    hosts.insert(name);
                }
                continue;
            }

            if let Some(val_expr) = entry.value() {
                if let Some(inner_set) = unwrap_attr_set(val_expr) {
                    for inner_entry in inner_set.attrpath_values() {
                        let Some(inner_path) = inner_entry.attrpath() else {
                            continue;
                        };
                        let mut inner_attrs = inner_path.attrs();
                        let Some(inner_first) = inner_attrs.next() else {
                            continue;
                        };
                        if let Some(name) = attr_to_name(&inner_first) {
                            hosts.insert(name);
                        }
                    }
                }
            }
            continue;
        }

        if attrs.next().is_some() {
            continue;
        }

        if let Some(val_expr) = entry.value() {
            if matches!(val_expr, ast::Expr::Lambda(_)) {
                continue;
            }
            hosts.insert(first_name);
        }
    }

    Ok(hosts)
}

pub fn diff_hosts(fleet_src: &str, keys_src: &str) -> Result<HostDiff> {
    let declared_set = extract_fleet_hosts(fleet_src)?;
    let keys_set = extract_key_hosts(keys_src)?;

    let declared = declared_set.iter().cloned().collect();
    let pending_install = declared_set.difference(&keys_set).cloned().collect();
    let installed = declared_set.intersection(&keys_set).cloned().collect();
    let removed = keys_set.difference(&declared_set).cloned().collect();

    Ok(HostDiff {
        declared,
        pending_install,
        installed,
        removed,
    })
}

pub fn diff_env(env_dir: &Path) -> Result<HostDiff> {
    let fleet_path = env_dir.join("fleet.nix");
    let keys_path = env_dir.join("keys.nix");

    let fleet_src = crate::fs::read_file(&fleet_path)?;
    let keys_src = if keys_path.exists() {
        crate::fs::read_file(&keys_path)?
    } else {
        "{}".to_string()
    };

    diff_hosts(&fleet_src, &keys_src).tap_ok(|diff| {
        tracing::debug!(
            env = %env_dir.display(),
            declared = ?diff.declared,
            pending_install = ?diff.pending_install,
            installed = ?diff.installed,
            removed = ?diff.removed,
            "calculated host diff"
        )
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::tempdir;

    #[test]
    fn test_diff_hosts_identifies_pending_install_installed_and_removed() {
        let fleet = r#"{
  hosts = {
    node-01 = {
      provider = "hetzner";
      ip = "100.64.0.1";
      services = [ "caddy" ];
    };
    node-02 = {
      provider = "hetzner";
      ip = "100.64.0.2";
      services = [ "api" ];
    };
  };
}"#;

        let keys = r#"{
  "node-01" = "ssh-ed25519 AAAAC3NzaC1lZDI1NTE5AAAAI...";
  "old-node" = "ssh-ed25519 AAAAC3NzaC1lZDI1NTE5AAAAI...";
}"#;

        let diff = diff_hosts(fleet, keys).expect("diff should succeed");
        assert_eq!(diff.declared, vec!["node-01", "node-02"]);
        assert_eq!(diff.pending_install, vec!["node-02"]);
        assert_eq!(diff.installed, vec!["node-01"]);
        assert_eq!(diff.removed, vec!["old-node"]);
    }

    #[test]
    fn test_diff_hosts_handles_dotted_host_entries() {
        let fleet = r#"{
  hosts."node-01" = {
    provider = "hetzner";
  };
  hosts.node-02 = {
    provider = "oracle";
  };
}"#;

        let keys = r#"{
  node-01 = "ssh-ed25519 AAAAC3NzaC1lZDI1NTE5AAAAI...";
}"#;

        let diff = diff_hosts(fleet, keys).expect("diff should succeed");
        assert_eq!(diff.declared, vec!["node-01", "node-02"]);
        assert_eq!(diff.pending_install, vec!["node-02"]);
        assert_eq!(diff.installed, vec!["node-01"]);
        assert!(diff.removed.is_empty());
    }

    #[test]
    fn test_diff_hosts_empty_cases() {
        let empty_fleet = "{ hosts = {}; }";
        let empty_keys = "{}";

        let diff = diff_hosts(empty_fleet, empty_keys).expect("empty diff should succeed");
        assert!(diff.declared.is_empty());
        assert!(diff.pending_install.is_empty());
        assert!(diff.installed.is_empty());
        assert!(diff.removed.is_empty());
    }

    #[test]
    fn test_diff_hosts_handles_let_in_and_lambda_wrappers() {
        let fleet = r#"{ ... }:
let
  common = { provider = "hetzner"; };
in
{
  hosts = {
    node-01 = common;
  };
}"#;

        let keys = r#"let
  key = "ssh-ed25519 AAAAC3NzaC1lZDI1NTE5AAAAI...";
in
{
  "node-01" = key;
}"#;

        let diff = diff_hosts(fleet, keys).expect("diff should succeed");
        assert_eq!(diff.declared, vec!["node-01"]);
        assert!(diff.pending_install.is_empty());
        assert_eq!(diff.installed, vec!["node-01"]);
        assert!(diff.removed.is_empty());
    }

    #[test]
    fn test_diff_hosts_handles_nested_host_keys() {
        let fleet = r#"{
  hosts = {
    node-01 = {};
  };
}"#;

        let keys = r#"{
  hostKeys = {
    "node-01" = "ssh-ed25519 AAAAC3NzaC1lZDI1NTE5AAAAI...";
  };
}"#;

        let diff = diff_hosts(fleet, keys).expect("diff should succeed");
        assert_eq!(diff.declared, vec!["node-01"]);
        assert!(diff.pending_install.is_empty());
        assert_eq!(diff.installed, vec!["node-01"]);
        assert!(diff.removed.is_empty());
    }

    #[test]
    fn test_diff_hosts_syntax_error_in_fleet() {
        let broken_fleet = "{ hosts = { node-01 = ; }; }";
        let keys = "{}";

        let result = diff_hosts(broken_fleet, keys);
        assert!(result.is_err());
    }

    #[test]
    fn test_diff_hosts_syntax_error_in_keys() {
        let fleet = "{ hosts = {}; }";
        let broken_keys = "{ node-01 = ; }";

        let result = diff_hosts(fleet, broken_keys);
        assert!(result.is_err());
    }

    #[test]
    fn test_diff_env_success() {
        let dir = tempdir().unwrap();
        let env_dir = dir.path();

        let fleet = r#"{
  hosts = {
    worker-01 = { provider = "hetzner"; };
  };
}"#;
        let keys = r#"{
  "worker-01" = "ssh-ed25519 AAAAC3NzaC1lZDI1NTE5AAAAI...";
}"#;

        std::fs::write(env_dir.join("fleet.nix"), fleet).unwrap();
        std::fs::write(env_dir.join("keys.nix"), keys).unwrap();

        let diff = diff_env(env_dir).expect("diff_env should succeed");
        assert_eq!(diff.declared, vec!["worker-01"]);
        assert_eq!(diff.installed, vec!["worker-01"]);
        assert!(diff.pending_install.is_empty());
        assert!(diff.removed.is_empty());
    }

    #[test]
    fn test_diff_hosts_ignores_helper_functions_and_dotted_non_hosts() {
        let fleet = r#"{
  hosts = {
    web-01 = {};
  };
}"#;
        let keys = r#"{
  mkKey = name: "ssh-ed25519 dummy";
  config.admin = "user";
  "web-01" = "ssh-ed25519 AAA1";
}"#;

        let diff = diff_hosts(fleet, keys).unwrap();
        assert_eq!(diff.declared, vec!["web-01"]);
        assert_eq!(diff.installed, vec!["web-01"]);
        assert!(diff.pending_install.is_empty());
        assert!(diff.removed.is_empty());
    }
}
