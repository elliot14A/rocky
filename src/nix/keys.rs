use std::path::Path;

use rnix::ast::{self, HasEntry};
use rowan::ast::AstNode;
use snafu::OptionExt;
use tap::prelude::*;

use crate::error::{Kind, NixParseSnafu, Result};
use crate::utils::validate::validate_name;

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

fn is_valid_public_key(key: &str) -> bool {
    let clean = key.trim();
    if clean.is_empty()
        || clean.contains('\n')
        || clean.contains('\r')
        || clean.contains('"')
        || clean.contains('$')
        || clean.contains('\\')
    {
        return false;
    }

    let parts: Vec<&str> = clean.split_whitespace().collect();
    if parts.len() < 2 {
        return false;
    }

    let key_type = parts[0];
    let key_data = parts[1];

    if !key_type
        .chars()
        .all(|c| c.is_ascii_alphanumeric() || c == '-' || c == '@' || c == '.')
    {
        return false;
    }

    if !key_data
        .chars()
        .all(|c| c.is_ascii_alphanumeric() || c == '+' || c == '/' || c == '=')
    {
        return false;
    }

    if parts.len() > 2 {
        let comment = parts[2..].join(" ");
        if comment
            .chars()
            .any(|c| c == '"' || c == '$' || c == '\\' || c.is_ascii_control())
        {
            return false;
        }
    }

    true
}

pub fn insert_host_key(
    content: &str,
    host_name: &str,
    public_key: &str,
    path: &Path,
) -> Result<String> {
    validate_name(Kind::Host, host_name)?;

    if !is_valid_public_key(public_key) {
        return NixParseSnafu {
            path: path.to_path_buf(),
            reason: format!("invalid public key format: '{}'", public_key),
        }
        .fail();
    }

    let clean_key = public_key.trim();

    let root_set = parse_root_attr_set(content, path)?;

    let mut host_keys_set: Option<ast::AttrSet> = None;

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

        if first_name == host_name && attrs.next().is_none() {
            let range = entry.syntax().text_range();
            let start = usize::from(range.start());
            let end = usize::from(range.end());
            let replacement = format!("\"{host_name}\" = \"{clean_key}\";");

            let mut updated = content[..start].to_string();
            updated.push_str(&replacement);
            updated.push_str(&content[end..]);
            return verify_syntax(updated, path);
        }

        if first_name == "hostKeys" {
            if let Some(second_attr) = attrs.next() {
                if attr_to_name(&second_attr).as_deref() == Some(host_name)
                    && attrs.next().is_none()
                {
                    let range = entry.syntax().text_range();
                    let start = usize::from(range.start());
                    let end = usize::from(range.end());
                    let replacement = format!("hostKeys.\"{host_name}\" = \"{clean_key}\";");

                    let mut updated = content[..start].to_string();
                    updated.push_str(&replacement);
                    updated.push_str(&content[end..]);
                    return verify_syntax(updated, path);
                }
            } else if attrs.next().is_none() {
                if let Some(val) = entry.value() {
                    if let Some(inner) = unwrap_attr_set(val) {
                        host_keys_set = Some(inner);
                    }
                }
            }
        }
    }

    if let Some(inner_set) = host_keys_set {
        for inner_entry in inner_set.attrpath_values() {
            let Some(inner_path) = inner_entry.attrpath() else {
                continue;
            };
            let mut inner_attrs = inner_path.attrs();
            let Some(inner_first) = inner_attrs.next() else {
                continue;
            };
            if attr_to_name(&inner_first).as_deref() == Some(host_name)
                && inner_attrs.next().is_none()
            {
                let range = inner_entry.syntax().text_range();
                let start = usize::from(range.start());
                let end = usize::from(range.end());
                let replacement = format!("\"{host_name}\" = \"{clean_key}\";");

                let mut updated = content[..start].to_string();
                updated.push_str(&replacement);
                updated.push_str(&content[end..]);
                return verify_syntax(updated, path);
            }
        }

        let r_curly = inner_set.r_curly_token().context(NixParseSnafu {
            path: path.to_path_buf(),
            reason: "missing closing brace for hostKeys attribute set",
        })?;
        let r_pos = usize::from(r_curly.text_range().start());
        let snippet = format!("  \"{host_name}\" = \"{clean_key}\";\n");

        let mut updated = content[..r_pos].to_string();
        if !updated.ends_with('\n') {
            updated.push('\n');
        }
        updated.push_str(&snippet);
        updated.push_str(&content[r_pos..]);
        return verify_syntax(updated, path);
    }

    let root_r_curly = root_set.r_curly_token().context(NixParseSnafu {
        path: path.to_path_buf(),
        reason: "missing closing brace for root attribute set",
    })?;
    let root_r_pos = usize::from(root_r_curly.text_range().start());
    let snippet = format!("  \"{host_name}\" = \"{clean_key}\";\n");

    let mut updated = content[..root_r_pos].to_string();
    if !updated.ends_with('\n') {
        updated.push('\n');
    }
    updated.push_str(&snippet);
    updated.push_str(&content[root_r_pos..]);
    verify_syntax(updated, path)
}

pub async fn register_host_key(
    keys_nix_path: &Path,
    host_name: &str,
    public_key: &str,
) -> Result<()> {
    let content = if keys_nix_path.exists() {
        crate::fs::read_file(keys_nix_path)?
    } else {
        "{\n}\n".to_string()
    };

    let updated = insert_host_key(&content, host_name, public_key, keys_nix_path)?;
    crate::fs::write_file(keys_nix_path, &updated)?;
    super::fmt([keys_nix_path]).await.tap_ok(|_| {
        tracing::debug!(
            host = %host_name,
            path = %keys_nix_path.display(),
            "registered host key in keys.nix"
        );
    })
}

pub fn extract_public_keys(content: &str, path: &Path) -> Result<Vec<String>> {
    let root_set = parse_root_attr_set(content, path)?;
    let mut keys = Vec::new();

    for entry in root_set.attrpath_values() {
        if let Some(ast::Expr::Str(s)) = entry.value() {
            let text = s.to_string();
            let clean = text.trim().trim_matches('"').trim();
            if is_valid_public_key(clean) {
                keys.push(clean.to_string());
            }
        }
    }

    Ok(keys)
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::tempdir;

    #[test]
    fn test_insert_host_key_empty_set() {
        let content = "{}";
        let path = Path::new("keys.nix");
        let updated = insert_host_key(content, "node-01", "ssh-ed25519 AAA1", path)
            .expect("insert should succeed");

        assert!(updated.contains(r#""node-01" = "ssh-ed25519 AAA1";"#));
    }

    #[test]
    fn test_insert_host_key_preserves_comments() {
        let content = r#"{
  # Existing host keys
  # Maps host name to ed25519 public key
}
"#;
        let path = Path::new("keys.nix");
        let updated = insert_host_key(content, "node-01", "ssh-ed25519 AAA1", path)
            .expect("insert should succeed");

        assert!(updated.contains("# Existing host keys"));
        assert!(updated.contains("# Maps host name to ed25519 public key"));
        assert!(updated.contains(r#""node-01" = "ssh-ed25519 AAA1";"#));
    }

    #[test]
    fn test_insert_host_key_appends_alongside_existing() {
        let content = r#"{
  "node-01" = "ssh-ed25519 AAA1";
}
"#;
        let path = Path::new("keys.nix");
        let updated = insert_host_key(content, "node-02", "ssh-ed25519 AAA2", path)
            .expect("insert should succeed");

        assert!(updated.contains(r#""node-01" = "ssh-ed25519 AAA1";"#));
        assert!(updated.contains(r#""node-02" = "ssh-ed25519 AAA2";"#));
    }

    #[test]
    fn test_insert_host_key_updates_existing_entry() {
        let content = r#"{
  "node-01" = "ssh-ed25519 OLD";
  "node-02" = "ssh-ed25519 AAA2";
}
"#;
        let path = Path::new("keys.nix");
        let updated = insert_host_key(content, "node-01", "ssh-ed25519 NEW", path)
            .expect("update should succeed");

        assert!(updated.contains(r#""node-01" = "ssh-ed25519 NEW";"#));
        assert!(updated.contains(r#""node-02" = "ssh-ed25519 AAA2";"#));
        assert!(!updated.contains("ssh-ed25519 OLD"));
    }

    #[test]
    fn test_insert_host_key_updates_unquoted_existing_entry() {
        let content = r#"{
  node-01 = "ssh-ed25519 OLD";
}
"#;
        let path = Path::new("keys.nix");
        let updated = insert_host_key(content, "node-01", "ssh-ed25519 NEW", path)
            .expect("update should succeed");

        assert!(updated.contains(r#""node-01" = "ssh-ed25519 NEW";"#));
        assert!(!updated.contains("ssh-ed25519 OLD"));
    }

    #[test]
    fn test_insert_host_key_nested_host_keys() {
        let content = r#"{
  hostKeys = {
    "node-01" = "ssh-ed25519 AAA1";
  };
}
"#;
        let path = Path::new("keys.nix");
        let updated = insert_host_key(content, "node-02", "ssh-ed25519 AAA2", path)
            .expect("insert should succeed");

        assert!(updated.contains(r#""node-01" = "ssh-ed25519 AAA1";"#));
        assert!(updated.contains(r#""node-02" = "ssh-ed25519 AAA2";"#));
    }

    #[test]
    fn test_insert_host_key_syntax_error() {
        let content = "{ broken = ; }";
        let path = Path::new("keys.nix");
        let result = insert_host_key(content, "node-01", "ssh-ed25519 AAA1", path);
        assert!(result.is_err());
    }

    #[test]
    fn test_insert_host_key_rejects_invalid_name() {
        let content = "{}";
        let path = Path::new("keys.nix");
        let result = insert_host_key(content, "../evil", "ssh-ed25519 AAA1", path);
        assert!(result.is_err());
    }

    #[test]
    fn test_insert_host_key_rejects_malformed_key() {
        let path = Path::new("keys.nix");
        assert!(insert_host_key("{}", "node-01", "", path).is_err());
        assert!(insert_host_key("{}", "node-01", "key\nwith\nnewlines", path).is_err());
        assert!(insert_host_key("{}", "node-01", "key\"with\"quotes", path).is_err());
        assert!(insert_host_key(
            "{}",
            "node-01",
            "ssh-ed25519 ${builtins.readFile /etc/passwd}",
            path
        )
        .is_err());
        assert!(insert_host_key("{}", "node-01", "ssh-ed25519 AAA1\\escaped", path).is_err());
        assert!(insert_host_key("{}", "node-01", "not_enough_parts", path).is_err());
    }

    #[test]
    fn test_insert_host_key_ignores_multi_segment_dotted_path() {
        let content = r#"{
  hostKeys."node-01".pubkey = "nested";
}
"#;
        let path = Path::new("keys.nix");
        let updated = insert_host_key(content, "node-01", "ssh-ed25519 AAA1", path)
            .expect("insert should succeed");
        assert!(updated.contains(r#"hostKeys."node-01".pubkey = "nested";"#));
        assert!(updated.contains(r#""node-01" = "ssh-ed25519 AAA1";"#));
    }

    #[test]
    fn test_insert_host_key_handles_let_in_and_lambda_wrappers() {
        let content = r#"{ ... }:
let
  common = "foo";
in
{
  "node-01" = "ssh-ed25519 OLD";
}
"#;
        let path = Path::new("keys.nix");
        let updated = insert_host_key(content, "node-01", "ssh-ed25519 NEW", path)
            .expect("update should succeed");
        assert!(updated.contains(r#""node-01" = "ssh-ed25519 NEW";"#));
        assert!(!updated.contains("ssh-ed25519 OLD"));
        assert!(updated.contains("common = \"foo\";"));
    }

    #[tokio::test]
    async fn test_register_host_key_file_io() {
        let dir = tempdir().unwrap();
        let keys_path = dir.path().join("keys.nix");

        std::fs::write(&keys_path, "{\n}\n").unwrap();

        register_host_key(&keys_path, "node-01", "ssh-ed25519 AAA1")
            .await
            .expect("register should succeed");

        let content = std::fs::read_to_string(&keys_path).unwrap();
        assert!(content.contains(r#""node-01" = "ssh-ed25519 AAA1";"#));
    }

    #[test]
    fn test_extract_public_keys() {
        let content = r#"{
  alice = "ssh-ed25519 AAAAC3NzaC1lZDI1NTE5AAAAI alice@laptop";
  bob = "ssh-rsa AAAAB3NzaC1yc2EAAAADAQABAAABAQC bob@work";
  invalid = "not-a-key";
}
"#;
        let keys = extract_public_keys(content, Path::new("keys.nix")).unwrap();
        assert_eq!(keys.len(), 2);
        assert!(keys[0].starts_with("ssh-ed25519"));
        assert!(keys[1].starts_with("ssh-rsa"));
    }
}
