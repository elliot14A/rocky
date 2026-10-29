use std::path::Path;

use rnix::ast::HasEntry;
use snafu::OptionExt;

use crate::error::{AlreadyExistsSnafu, Kind, NixParseSnafu, Result};
use crate::nix::ast::{attr_to_name, parse_root_attr_set};
use crate::validate::validate_name;

pub fn insert_secret_entry(
    secrets_nix_content: &str,
    secret_name: &str,
    services: &[String],
    all_hosts: bool,
) -> Result<String> {
    let clean_name = secret_name.strip_suffix(".age").unwrap_or(secret_name);
    validate_name(Kind::Secret, clean_name)?;
    for svc in services {
        validate_name(Kind::Service, svc)?;
    }

    let file_key = format!("{clean_name}.age");
    let base_name = clean_name;

    let path = Path::new("secrets.nix");
    let root_set = parse_root_attr_set(secrets_nix_content, path)?;

    for entry in root_set.attrpath_values() {
        let Some(attrpath) = entry.attrpath() else {
            continue;
        };
        let mut attrs = attrpath.attrs();
        let Some(first_attr) = attrs.next() else {
            continue;
        };
        if let Some(entry_name) = attr_to_name(&first_attr) {
            let entry_base = entry_name.strip_suffix(".age").unwrap_or(&entry_name);
            if entry_name == file_key || entry_base == base_name {
                return AlreadyExistsSnafu {
                    kind: Kind::Secret,
                    name: base_name.to_string(),
                    path: path.to_path_buf(),
                }
                .fail();
            }
        }
    }

    let snippet = if all_hosts {
        format!("  \"{file_key}\".publicKeys = allHosts;\n")
    } else if services.is_empty() {
        format!("  \"{file_key}\".publicKeys = recipientsFor [ ];\n")
    } else if services.len() == 1 {
        format!(
            "  \"{file_key}\".publicKeys = recipientsFor [ \"{}\" ];\n",
            services[0]
        )
    } else {
        let mut s = format!("  \"{file_key}\".publicKeys = recipientsFor [\n");
        for svc in services {
            s.push_str(&format!("    \"{svc}\"\n"));
        }
        s.push_str("  ];\n");
        s
    };

    let r_curly = root_set.r_curly_token().context(NixParseSnafu {
        path: path.to_path_buf(),
        reason: "missing closing brace for attribute set",
    })?;
    let r_pos = usize::from(r_curly.text_range().start());

    let mut updated = secrets_nix_content[..r_pos].to_string();
    if !updated.ends_with('\n') {
        updated.push('\n');
    }
    updated.push_str(&snippet);
    updated.push_str(&secrets_nix_content[r_pos..]);

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

pub fn list_declared_secrets(secrets_nix_content: &str) -> Result<Vec<String>> {
    let path = Path::new("secrets.nix");
    let root_set = parse_root_attr_set(secrets_nix_content, path)?;
    let mut names = Vec::new();

    for entry in root_set.attrpath_values() {
        let Some(attrpath) = entry.attrpath() else {
            continue;
        };
        let mut attrs = attrpath.attrs();
        let Some(first_attr) = attrs.next() else {
            continue;
        };
        if let Some(entry_name) = attr_to_name(&first_attr) {
            names.push(entry_name);
        }
    }

    Ok(names)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::error::RockyError;

    #[test]
    fn test_insert_secret_entry_empty_set() {
        let content = "{\n}\n";
        let res = insert_secret_entry(content, "jwt", &["postgrest".to_string()], false).unwrap();
        assert!(res.contains(r#""jwt.age".publicKeys = recipientsFor [ "postgrest" ];"#));
    }

    #[test]
    fn test_insert_secret_entry_with_services() {
        let content = r#"let
  inherit (import ../../../lib/secrets.nix { inherit operators fleet hostKeys; }) recipientsFor allHosts;
in
{
}
"#;
        let services = vec!["postgrest".to_string(), "storage".to_string()];
        let res = insert_secret_entry(content, "jwt.age", &services, false).unwrap();
        assert!(res.contains(r#""jwt.age".publicKeys = recipientsFor ["#));
        assert!(res.contains(r#""postgrest""#));
        assert!(res.contains(r#""storage""#));
    }

    #[test]
    fn test_insert_secret_entry_all_hosts() {
        let content = "{\n}\n";
        let res = insert_secret_entry(content, "tailscale", &[], true).unwrap();
        assert!(res.contains(r#""tailscale.age".publicKeys = allHosts;"#));
    }

    #[test]
    fn test_insert_secret_entry_empty_services() {
        let content = "{\n}\n";
        let res = insert_secret_entry(content, "orphan", &[], false).unwrap();
        assert!(res.contains(r#""orphan.age".publicKeys = recipientsFor [ ];"#));
    }

    #[test]
    fn test_insert_secret_entry_invalid_syntax_rejected() {
        let content = "this is not valid nix {";
        let res = insert_secret_entry(content, "jwt", &[], false);
        assert!(matches!(res, Err(RockyError::NixParse { .. })));
    }

    #[test]
    fn test_insert_secret_entry_preserves_comments() {
        let content = r#"# Top-level comment about fleet secrets
let
  operators = import ./keys.nix;
in
{
  # Existing secret comment
  "existing.age".publicKeys = allHosts;
}
"#;
        let res = insert_secret_entry(content, "new-secret", &["api".to_string()], false).unwrap();
        assert!(res.starts_with("# Top-level comment about fleet secrets"));
        assert!(res.contains("# Existing secret comment"));
        assert!(res.contains(r#""existing.age".publicKeys = allHosts;"#));
        assert!(res.contains(r#""new-secret.age".publicKeys = recipientsFor [ "api" ];"#));
    }

    #[test]
    fn test_insert_secret_entry_rejects_invalid_service_name() {
        let content = "{ }\n";
        let res = insert_secret_entry(content, "jwt", &["invalid; name".to_string()], false);
        assert!(matches!(
            res,
            Err(RockyError::InvalidName {
                kind: Kind::Service,
                ..
            })
        ));
    }

    #[test]
    fn test_insert_secret_entry_rejects_duplicate() {
        let content = r#"{
  "jwt.age".publicKeys = allHosts;
}
"#;
        let res = insert_secret_entry(content, "jwt", &[], false);
        assert!(matches!(
            res,
            Err(RockyError::AlreadyExists {
                kind: Kind::Secret,
                name,
                ..
            }) if name == "jwt"
        ));

        let res_with_ext = insert_secret_entry(content, "jwt.age", &[], false);
        assert!(matches!(
            res_with_ext,
            Err(RockyError::AlreadyExists {
                kind: Kind::Secret,
                name,
                ..
            }) if name == "jwt"
        ));
    }

    #[test]
    fn test_list_declared_secrets() {
        let content = r#"{
  "jwt.age".publicKeys = allHosts;
  "db.age".publicKeys = recipientsFor [ "postgres" ];
}
"#;
        let secrets = list_declared_secrets(content).unwrap();
        assert_eq!(secrets, vec!["jwt.age".to_string(), "db.age".to_string()]);
    }
}
