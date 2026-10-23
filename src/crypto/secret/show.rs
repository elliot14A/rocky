use std::path::Path;

use snafu::{ensure, OptionExt};
use tap::prelude::*;

use crate::crypto::age;
use crate::crypto::secret::format::extract_key;
use crate::error::{Kind, NotFoundSnafu, Result, RockyError};
use crate::utils::validate::validate_name;
use crate::{fs, nix};

pub async fn show_secret(
    env_dir: &Path,
    name: &str,
    identity: Option<&Path>,
    key_filter: Option<&str>,
) -> Result<String> {
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
    let file_name = format!("{clean_name}.age");

    let secrets_dir = nix::resolve_secrets_dir(env_dir);

    let secret_path = secrets_dir.join(&file_name);
    ensure!(
        secret_path.is_file(),
        NotFoundSnafu {
            kind: Kind::Secret,
            name: file_name,
            path: secret_path.clone(),
        }
    );

    let ciphertext = fs::read_file_bytes(&secret_path)?;
    let decrypted = age::decrypt(&ciphertext, identity).await?;
    let content = String::from_utf8(decrypted).map_err(|e| RockyError::Io {
        path: secret_path.clone(),
        source: std::io::Error::new(std::io::ErrorKind::InvalidData, e),
    })?;

    if let Some(key) = key_filter {
        extract_key(&content, key)?.context(NotFoundSnafu {
            kind: Kind::Secret,
            name: format!("{clean_name}:{key}"),
            path: secret_path,
        })
    } else {
        Ok(content)
    }
    .tap_ok(|_| {
        tracing::debug!(name = %clean_name, "retrieved secret");
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn test_show_secret_nonexistent_secret_fails() {
        let dir = tempfile::tempdir().unwrap();
        let res = show_secret(dir.path(), "missing", None, None).await;
        assert!(matches!(res, Err(RockyError::NotFound { .. })));
    }

    #[tokio::test]
    async fn test_show_secret_path_traversal_fails() {
        let dir = tempfile::tempdir().unwrap();
        let res = show_secret(dir.path(), "../../evil", None, None).await;
        assert!(matches!(
            res,
            Err(RockyError::InvalidName {
                kind: Kind::Secret,
                ..
            })
        ));
    }
}
