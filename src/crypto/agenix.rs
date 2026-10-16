use std::path::Path;

use snafu::ensure;

use crate::error::{Kind, NotADirectorySnafu, NotFoundSnafu, Result};
use crate::process::exec;

pub fn build_rekey_args(identity: Option<&Path>) -> Vec<String> {
    let mut args = vec!["--rekey".to_string()];
    if let Some(id) = identity {
        args.push("-i".to_string());
        args.push(id.to_string_lossy().to_string());
    }
    args
}

pub async fn rekey(secrets_dir: &Path, identity: Option<&Path>) -> Result<()> {
    ensure!(
        secrets_dir.exists(),
        NotFoundSnafu {
            kind: Kind::Secret,
            name: "secrets".to_string(),
            path: secrets_dir.to_path_buf(),
        }
    );

    ensure!(
        secrets_dir.is_dir(),
        NotADirectorySnafu {
            path: secrets_dir.to_path_buf(),
        }
    );

    let secrets_nix = secrets_dir.join("secrets.nix");
    ensure!(
        secrets_nix.is_file(),
        NotFoundSnafu {
            kind: Kind::Secret,
            name: "secrets.nix".to_string(),
            path: secrets_nix,
        }
    );

    let abs_identity = match identity {
        Some(id) => {
            ensure!(
                id.is_file(),
                NotFoundSnafu {
                    kind: Kind::Secret,
                    name: id.display().to_string(),
                    path: id.to_path_buf(),
                }
            );
            let abs = std::fs::canonicalize(id).unwrap_or_else(|_| {
                if id.is_relative() {
                    std::env::current_dir()
                        .map(|cwd| cwd.join(id))
                        .unwrap_or_else(|_| id.to_path_buf())
                } else {
                    id.to_path_buf()
                }
            });
            Some(abs)
        }
        None => None,
    };

    let args = build_rekey_args(abs_identity.as_deref());

    exec("agenix", &args, Some(secrets_dir)).await?;

    tracing::debug!(
        secrets_dir = %secrets_dir.display(),
        identity = ?identity.map(|p| p.display()),
        "rekeyed agenix secrets"
    );

    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::tempdir;

    #[test]
    fn test_build_rekey_args_without_identity() {
        let args = build_rekey_args(None);
        assert_eq!(args, vec!["--rekey"]);
    }

    #[test]
    fn test_build_rekey_args_with_identity() {
        let identity = Path::new("/home/user/.ssh/id_ed25519");
        let args = build_rekey_args(Some(identity));
        assert_eq!(args, vec!["--rekey", "-i", "/home/user/.ssh/id_ed25519"]);
    }

    #[tokio::test]
    async fn test_rekey_nonexistent_directory_fails() {
        let path = Path::new("/nonexistent/secrets/dir");
        let result = rekey(path, None).await;
        assert!(result.is_err());
    }

    #[tokio::test]
    async fn test_rekey_missing_secrets_nix_fails() {
        let dir = tempdir().unwrap();
        let result = rekey(dir.path(), None).await;
        assert!(result.is_err());
    }

    #[tokio::test]
    async fn test_rekey_file_instead_of_directory_fails() {
        let dir = tempdir().unwrap();
        let file_path = dir.path().join("not_a_dir");
        std::fs::write(&file_path, "content").unwrap();

        let result = rekey(&file_path, None).await;
        assert!(result.is_err());
    }

    #[tokio::test]
    async fn test_rekey_nonexistent_identity_fails() {
        let dir = tempdir().unwrap();
        std::fs::write(dir.path().join("secrets.nix"), "{}\n").unwrap();
        let id_path = dir.path().join("nonexistent_key");

        let result = rekey(dir.path(), Some(&id_path)).await;
        assert!(result.is_err());
    }
}
