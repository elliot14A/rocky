use std::os::unix::fs::PermissionsExt;

use snafu::ResultExt;
use tap::prelude::*;

use crate::error::{IoSnafu, Kind, Result};
use crate::process::exec;
use crate::validate::validate_name;

pub use crate::fs::canonicalize_identity;

pub const HOST_KEY_FILE: &str = "ssh_host_ed25519_key";
pub const HOST_PUB_KEY_FILE: &str = "ssh_host_ed25519_key.pub";

#[derive(Debug)]
pub struct GeneratedKey {
    pub public_key: String,
    pub extra_files_dir: tempfile::TempDir,
}

pub async fn generate_host_key(host_name: &str) -> Result<GeneratedKey> {
    validate_name(Kind::Host, host_name)?;

    let temp_dir = tempfile::tempdir().context(IoSnafu {
        path: std::env::temp_dir(),
    })?;

    let ssh_dir = temp_dir.path().join("etc/ssh");
    crate::fs::ensure_dir(&ssh_dir)?;

    let priv_key_path = ssh_dir.join(HOST_KEY_FILE);
    let pub_key_path = ssh_dir.join(HOST_PUB_KEY_FILE);
    let comment = format!("host-key:{host_name}");

    let priv_key_str = priv_key_path.to_string_lossy().to_string();
    let args = [
        "-t",
        "ed25519",
        "-N",
        "",
        "-C",
        &comment,
        "-f",
        &priv_key_str,
    ];

    exec("ssh-keygen", &args, None).await?;

    std::fs::set_permissions(&priv_key_path, std::fs::Permissions::from_mode(0o600)).context(
        IoSnafu {
            path: &priv_key_path,
        },
    )?;

    std::fs::set_permissions(&pub_key_path, std::fs::Permissions::from_mode(0o644)).context(
        IoSnafu {
            path: &pub_key_path,
        },
    )?;

    let public_key = crate::fs::read_file(&pub_key_path)?
        .trim()
        .to_string()
        .tap(|key| {
            tracing::debug!(
                host = %host_name,
                public_key = %key,
                "generated host ssh key"
            )
        });

    Ok(GeneratedKey {
        public_key,
        extra_files_dir: temp_dir,
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn test_generate_host_key_success() {
        let key = generate_host_key("node-01")
            .await
            .expect("key generation should succeed");

        assert!(key.public_key.starts_with("ssh-ed25519 "));
        assert!(key.public_key.ends_with("host-key:node-01"));

        let base_dir = key.extra_files_dir.path();
        let etc_dir = base_dir.join("etc");
        let ssh_dir = base_dir.join("etc/ssh");
        let priv_key_path = ssh_dir.join(HOST_KEY_FILE);
        let pub_key_path = ssh_dir.join(HOST_PUB_KEY_FILE);

        assert!(priv_key_path.is_file());
        assert!(pub_key_path.is_file());

        let priv_perms = std::fs::metadata(&priv_key_path)
            .unwrap()
            .permissions()
            .mode();
        assert_eq!(priv_perms & 0o777, 0o600);

        let pub_perms = std::fs::metadata(&pub_key_path)
            .unwrap()
            .permissions()
            .mode();
        assert_eq!(pub_perms & 0o777, 0o644);

        let etc_perms = std::fs::metadata(&etc_dir).unwrap().permissions().mode();
        assert_eq!(etc_perms & 0o111, 0o111);

        let ssh_perms = std::fs::metadata(&ssh_dir).unwrap().permissions().mode();
        assert_eq!(ssh_perms & 0o111, 0o111);
    }

    #[tokio::test]
    async fn test_generate_host_key_cryptographic_roundtrip() {
        let key = generate_host_key("db-01")
            .await
            .expect("key generation should succeed");

        let priv_key_path = key
            .extra_files_dir
            .path()
            .join("etc/ssh")
            .join(HOST_KEY_FILE);

        let derived_pub = exec(
            "ssh-keygen",
            &["-y", "-f", &priv_key_path.to_string_lossy()],
            None,
        )
        .await
        .expect("ssh-keygen -y should derive public key from private key");

        assert_eq!(derived_pub.trim(), key.public_key.trim());
    }

    #[tokio::test]
    async fn test_generate_host_key_openssh_format() {
        let key = generate_host_key("web-01")
            .await
            .expect("key generation should succeed");

        let priv_content = std::fs::read_to_string(
            key.extra_files_dir
                .path()
                .join("etc/ssh")
                .join(HOST_KEY_FILE),
        )
        .expect("should read private key");

        assert!(priv_content.starts_with("-----BEGIN OPENSSH PRIVATE KEY-----"));
        assert!(priv_content
            .trim()
            .ends_with("-----END OPENSSH PRIVATE KEY-----"));

        let tokens: Vec<&str> = key.public_key.split_whitespace().collect();
        assert_eq!(tokens.len(), 3);
        assert_eq!(tokens[0], "ssh-ed25519");
        assert_eq!(tokens[2], "host-key:web-01");
        assert_eq!(tokens[1].len(), 68);
    }

    #[tokio::test]
    async fn test_generate_host_key_rejects_malicious_names() {
        let invalid_names = [
            "",
            "-dash-start",
            "--flag",
            "../traversal",
            "has space",
            "semi;colon",
            "shell$(whoami)",
            "pipe|cmd",
            "slash/in/name",
        ];

        for name in invalid_names {
            let result = generate_host_key(name).await;
            assert!(
                result.is_err(),
                "generate_host_key should reject invalid name: '{}'",
                name
            );
        }
    }

    #[tokio::test]
    async fn test_generate_host_key_cleaned_up_on_drop() {
        let path = {
            let key = generate_host_key("node-temp")
                .await
                .expect("key generation should succeed");
            let path = key.extra_files_dir.path().to_path_buf();
            assert!(path.exists());
            path
        };
        assert!(!path.exists());
    }

    #[tokio::test]
    async fn test_generate_host_key_isolation_between_hosts() {
        let key1 = generate_host_key("node-alpha")
            .await
            .expect("key1 generation should succeed");
        let key2 = generate_host_key("node-beta")
            .await
            .expect("key2 generation should succeed");

        assert_ne!(key1.public_key, key2.public_key);
        assert_ne!(key1.extra_files_dir.path(), key2.extra_files_dir.path());
    }

    #[test]
    fn test_canonicalize_identity() {
        assert_eq!(canonicalize_identity(None).unwrap(), None);

        let temp = tempfile::tempdir().unwrap();
        let key_file = temp.path().join("id_ed25519");
        std::fs::write(&key_file, "key-content").unwrap();
        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt;
            std::fs::set_permissions(&key_file, std::fs::Permissions::from_mode(0o600)).unwrap();
        }

        let resolved = canonicalize_identity(Some(&key_file)).unwrap();
        assert!(resolved.is_some());
        assert!(resolved.unwrap().is_absolute());

        let insecure_key = temp.path().join("id_insecure");
        std::fs::write(&insecure_key, "key-content").unwrap();
        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt;
            std::fs::set_permissions(&insecure_key, std::fs::Permissions::from_mode(0o644)).unwrap();
            let res = canonicalize_identity(Some(&insecure_key));
            assert!(res.is_err());
            match res.unwrap_err() {
                crate::error::RockyError::InsecurePath { .. } => {}
                other => panic!("expected InsecurePath, got: {:?}", other),
            }
        }

        let missing = temp.path().join("missing");
        assert!(canonicalize_identity(Some(&missing)).is_err());

        let evil_quote = temp.path().join("id_\"evil");
        assert!(canonicalize_identity(Some(&evil_quote)).is_err());

        let evil_dollar = temp.path().join("id_$evil");
        assert!(canonicalize_identity(Some(&evil_dollar)).is_err());
    }
}
