use std::path::{Path, PathBuf};

use tap::prelude::*;

use super::eval_secret_recipients;
use crate::crypto::age;
use crate::error::{Kind, Result};
use crate::validate::validate_name;
use crate::{fs, nix};

pub async fn generate_mock_secrets(fleet_root: &Path) -> Result<Vec<PathBuf>> {
    let envs_dir = fleet_root.join("environments");
    if !envs_dir.is_dir() {
        return Ok(Vec::new());
    }

    let mut mocked = Vec::new();

    let entries = std::fs::read_dir(&envs_dir).map_err(|e| crate::error::RockyError::Io {
        path: envs_dir.clone(),
        source: e,
    })?;

    for entry in entries.filter_map(|e| e.ok()) {
        let env_dir = entry.path();
        if !env_dir.is_dir() {
            continue;
        }

        let secrets_dir = nix::resolve_secrets_dir(&env_dir);

        let secrets_nix = secrets_dir.join("secrets.nix");
        if !secrets_nix.is_file() {
            continue;
        }

        let content = fs::read_file(&secrets_nix)?;
        let declared = nix::list_declared_secrets(&content)?;

        for secret_file in declared {
            let clean_name = secret_file.strip_suffix(".age").unwrap_or(&secret_file);
            if validate_name(Kind::Secret, clean_name).is_err() {
                tracing::warn!(
                    secret = %secret_file,
                    "skipping declared secret with invalid name in secrets.nix"
                );
                continue;
            }
            let valid_file_name = format!("{clean_name}.age");
            let secret_path = secrets_dir.join(&valid_file_name);
            if secret_path.exists() {
                continue;
            }

            let recipients = eval_secret_recipients(&secrets_dir, &valid_file_name)
                .await
                .unwrap_or_default();

            if !recipients.is_empty() {
                if let Ok(ciphertext) = age::encrypt(b"MOCK_SECRET=true\n", &recipients).await {
                    fs::write_file_bytes(&secret_path, &ciphertext)?;
                    mocked.push(secret_path);
                    continue;
                }
            }

            // Fallback mock age ciphertext
            let fallback_mock = b"age-encryption.org/v1\n-> mock\n---\nmock-ciphertext\n";
            fs::write_file_bytes(&secret_path, fallback_mock)?;
            mocked.push(secret_path);
        }
    }

    Ok(mocked).tap_ok(|list| {
        tracing::debug!(count = list.len(), "generated mock secrets");
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn test_generate_mock_secrets_creates_missing_files() {
        let dir = tempfile::tempdir().unwrap();
        let fleet_root = dir.path();
        let env_dir = fleet_root.join("environments").join("prod");
        std::fs::create_dir_all(&env_dir).unwrap();

        let secrets_nix = env_dir.join("secrets.nix");
        let content = r#"{
  "missing1.age".publicKeys = [ ];
  "missing2.age".publicKeys = [ ];
}
"#;
        std::fs::write(&secrets_nix, content).unwrap();

        let mocked = generate_mock_secrets(fleet_root).await.unwrap();
        assert_eq!(mocked.len(), 2);
        assert!(env_dir.join("missing1.age").is_file());
        assert!(env_dir.join("missing2.age").is_file());

        // Second run should find 0 missing
        let mocked2 = generate_mock_secrets(fleet_root).await.unwrap();
        assert!(mocked2.is_empty());
    }

    #[tokio::test]
    async fn test_generate_mock_secrets_ignores_path_traversal_entries() {
        let dir = tempfile::tempdir().unwrap();
        let fleet_root = dir.path();
        let env_dir = fleet_root.join("environments").join("prod");
        std::fs::create_dir_all(&env_dir).unwrap();

        let secrets_nix = env_dir.join("secrets.nix");
        let content = r#"{
  "valid.age".publicKeys = [ ];
  "../../evil.age".publicKeys = [ ];
  "/etc/shadow.age".publicKeys = [ ];
}
"#;
        std::fs::write(&secrets_nix, content).unwrap();

        let mocked = generate_mock_secrets(fleet_root).await.unwrap();
        assert_eq!(mocked.len(), 1);
        assert!(env_dir.join("valid.age").is_file());
        assert!(!fleet_root.join("evil.age").exists());
    }
}
