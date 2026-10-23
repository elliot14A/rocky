use std::path::{Path, PathBuf};

use snafu::ensure;
use tap::prelude::*;

use super::eval_secret_recipients;
use crate::crypto::age;
use crate::error::{AlreadyExistsSnafu, Kind, NotFoundSnafu, Result};
use crate::fs;
use crate::nix;
use crate::utils::validate::validate_name;

pub async fn create_secret(
    env_dir: &Path,
    name: &str,
    content: &str,
    services: &[String],
    all_hosts: bool,
) -> Result<PathBuf> {
    ensure!(
        env_dir.is_dir(),
        NotFoundSnafu {
            kind: Kind::Environment,
            name: env_dir.display().to_string(),
            path: env_dir.to_path_buf(),
        }
    );

    let clean_name = name.strip_suffix(".age").unwrap_or(name);
    validate_name(Kind::Secret, clean_name)?;

    let fleet_root = env_dir.parent().and_then(|p| p.parent());
    let services_dir = fleet_root.map(|r| r.join("services"));

    if !services.is_empty() {
        for svc in services {
            validate_name(Kind::Service, svc)?;
        }

        let dir = services_dir.ok_or_else(|| {
            NotFoundSnafu {
                kind: Kind::Service,
                name: "services".to_string(),
                path: env_dir.to_path_buf(),
            }
            .build()
        })?;
        ensure!(
            dir.is_dir(),
            NotFoundSnafu {
                kind: Kind::Service,
                name: "services".to_string(),
                path: dir.clone(),
            }
        );

        for svc in services {
            let svc_path = dir.join(format!("{svc}.nix"));
            ensure!(
                svc_path.is_file(),
                NotFoundSnafu {
                    kind: Kind::Service,
                    name: svc.clone(),
                    path: svc_path,
                }
            );
        }
    }
    let file_name = format!("{clean_name}.age");

    let secrets_dir = nix::resolve_secrets_dir(env_dir);
    let secrets_nix = secrets_dir.join("secrets.nix");
    ensure!(
        secrets_nix.is_file(),
        NotFoundSnafu {
            kind: Kind::Secret,
            name: "secrets.nix".to_string(),
            path: secrets_nix.clone(),
        }
    );

    let secret_path = secrets_dir.join(&file_name);
    ensure!(
        !secret_path.exists(),
        AlreadyExistsSnafu {
            kind: Kind::Secret,
            name: file_name.clone(),
            path: secret_path.clone(),
        }
    );

    let original_content = fs::read_file(&secrets_nix)?;
    let updated_content =
        nix::insert_secret_entry(&original_content, clean_name, services, all_hosts)?;

    let mut rollback = fs::Rollback::new();
    rollback.restore_file_on_failure(&secrets_nix, &original_content);
    rollback.remove_file_on_failure(&secret_path);

    fs::write_file(&secrets_nix, &updated_content)?;
    nix::fmt([&secrets_nix]).await?;

    let recipients = eval_secret_recipients(&secrets_dir, &file_name).await?;
    let ciphertext = age::encrypt(content.as_bytes(), &recipients).await?;
    fs::write_file_bytes(&secret_path, &ciphertext)?;

    rollback.commit();
    Ok(secret_path).tap_ok(|path| {
        tracing::debug!(path = %path.display(), "created secret");
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn test_create_secret_rejects_invalid_service_name() {
        let dir = tempfile::tempdir().unwrap();
        let env_dir = dir.path().join("prod");
        std::fs::create_dir_all(&env_dir).unwrap();
        let secrets_nix = env_dir.join("secrets.nix");
        std::fs::write(&secrets_nix, "{ }").unwrap();

        let res = create_secret(
            &env_dir,
            "api-token",
            "KEY=val\n",
            &["bad service".to_string()],
            false,
        )
        .await;
        assert!(matches!(
            res,
            Err(crate::error::RockyError::InvalidName {
                kind: Kind::Service,
                ..
            })
        ));
    }

    #[tokio::test]
    async fn test_create_secret_rejects_nonexistent_service() {
        let dir = tempfile::tempdir().unwrap();
        let fleet_root = dir.path();
        let env_dir = fleet_root.join("environments").join("prod");
        let services_dir = fleet_root.join("services");
        std::fs::create_dir_all(&env_dir).unwrap();
        std::fs::create_dir_all(&services_dir).unwrap();
        let secrets_nix = env_dir.join("secrets.nix");
        std::fs::write(&secrets_nix, "{ }").unwrap();

        let res = create_secret(
            &env_dir,
            "api-token",
            "KEY=val\n",
            &["missing-svc".to_string()],
            false,
        )
        .await;
        assert!(matches!(
            res,
            Err(crate::error::RockyError::NotFound {
                kind: Kind::Service,
                ..
            })
        ));
    }

    #[tokio::test]
    async fn test_create_secret_rejects_when_services_dir_missing() {
        let dir = tempfile::tempdir().unwrap();
        let fleet_root = dir.path();
        let env_dir = fleet_root.join("environments").join("prod");
        std::fs::create_dir_all(&env_dir).unwrap();
        let secrets_nix = env_dir.join("secrets.nix");
        std::fs::write(&secrets_nix, "{ }").unwrap();

        let res = create_secret(
            &env_dir,
            "api-token",
            "KEY=val\n",
            &["postgrest".to_string()],
            false,
        )
        .await;
        assert!(matches!(
            res,
            Err(crate::error::RockyError::NotFound {
                kind: Kind::Service,
                ..
            })
        ));
    }

    #[tokio::test]
    async fn test_create_secret_allows_script_with_assignments_without_syntax_error() {
        let dir = tempfile::tempdir().unwrap();
        let env_dir = dir.path().join("prod");
        std::fs::create_dir_all(&env_dir).unwrap();
        let secrets_nix = env_dir.join("secrets.nix");
        std::fs::write(&secrets_nix, "{ }").unwrap();

        let script_content = "#!/bin/sh\nPORT=8080\necho done\n";
        let res = create_secret(&env_dir, "test-script", script_content, &[], false).await;
        // Script contains '=' but is not a KV file; it should not be rejected by KV syntax validation
        assert!(!matches!(
            res,
            Err(crate::error::RockyError::InvalidSecretSyntax { .. })
        ));
    }

    #[tokio::test]
    async fn test_create_secret_rejects_already_existing_secret_file() {
        let dir = tempfile::tempdir().unwrap();
        let env_dir = dir.path().join("prod");
        std::fs::create_dir_all(&env_dir).unwrap();
        let secrets_nix = env_dir.join("secrets.nix");
        std::fs::write(&secrets_nix, "{ }").unwrap();
        std::fs::write(env_dir.join("existing.age"), b"dummy").unwrap();

        let res = create_secret(&env_dir, "existing", "KEY=val", &[], false).await;
        assert!(matches!(
            res,
            Err(crate::error::RockyError::AlreadyExists { .. })
        ));
    }

    #[tokio::test]
    async fn test_create_and_show_secret_roundtrip() {
        let dir = tempfile::tempdir().unwrap();
        let env_dir = dir.path().join("prod");
        let secrets_dir = env_dir.join("secrets");
        std::fs::create_dir_all(&secrets_dir).unwrap();

        let key_path = dir.path().join("id_ed25519");
        let status = std::process::Command::new("ssh-keygen")
            .args(["-t", "ed25519", "-N", "", "-f", key_path.to_str().unwrap()])
            .output()
            .unwrap();
        assert!(status.status.success());
        let pubkey = std::fs::read_to_string(dir.path().join("id_ed25519.pub")).unwrap();

        let secrets_nix = secrets_dir.join("secrets.nix");
        std::fs::write(
            &secrets_nix,
            format!("let allHosts = [ \"{}\" ]; in {{\n}}\n", pubkey.trim()),
        )
        .unwrap();

        let secret_path = create_secret(
            &env_dir,
            "supabase-jwt",
            "JWT_SECRET=supersecrettoken123\nANON_KEY=anonkey456\n",
            &[],
            true,
        )
        .await
        .unwrap();

        assert!(secret_path.is_file());
        assert_eq!(secret_path.file_name().unwrap(), "supabase-jwt.age");

        let secrets_nix_content = std::fs::read_to_string(&secrets_nix).unwrap();
        assert!(secrets_nix_content.contains("\"supabase-jwt.age\".publicKeys = allHosts;"));

        let full_content =
            crate::crypto::secret::show_secret(&env_dir, "supabase-jwt", Some(&key_path), None)
                .await
                .unwrap();
        assert!(full_content.contains("JWT_SECRET=supersecrettoken123"));

        let filtered = crate::crypto::secret::show_secret(
            &env_dir,
            "supabase-jwt",
            Some(&key_path),
            Some("JWT_SECRET"),
        )
        .await
        .unwrap();
        assert_eq!(filtered, "supersecrettoken123");

        let anon = crate::crypto::secret::show_secret(
            &env_dir,
            "supabase-jwt",
            Some(&key_path),
            Some("ANON_KEY"),
        )
        .await
        .unwrap();
        assert_eq!(anon, "anonkey456");

        let missing = crate::crypto::secret::show_secret(
            &env_dir,
            "supabase-jwt",
            Some(&key_path),
            Some("NON_EXISTENT"),
        )
        .await;
        assert!(matches!(
            missing,
            Err(crate::error::RockyError::NotFound { .. })
        ));
    }
}
