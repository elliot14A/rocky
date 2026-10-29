use std::path::{Path, PathBuf};

use snafu::{ensure, ResultExt};

use crate::error::{InsecurePathSnafu, IoSnafu, Kind, NotFoundSnafu, Result};

pub fn canonicalize_identity(identity: Option<&Path>) -> Result<Option<PathBuf>> {
    let Some(id) = identity else {
        return Ok(None);
    };

    let id_str = id.to_string_lossy();
    ensure!(
        !id_str.chars().any(|c| c == '"'
            || c == '\''
            || c == '\\'
            || c == '$'
            || c == '`'
            || c == '\n'
            || c == '\r'),
        InsecurePathSnafu {
            path: id.to_path_buf(),
            reason: "identity path cannot contain quotes, shell metacharacters, or newlines"
                .to_string(),
        }
    );

    let path_str = id.to_string_lossy();
    let expanded = if path_str.starts_with("~/") {
        if let Ok(home) = std::env::var("HOME") {
            PathBuf::from(home).join(path_str.trim_start_matches("~/"))
        } else {
            id.to_path_buf()
        }
    } else {
        id.to_path_buf()
    };

    ensure!(
        expanded.is_file(),
        NotFoundSnafu {
            kind: Kind::Secret,
            name: id.display().to_string(),
            path: id.to_path_buf(),
        }
    );

    let abs = std::fs::canonicalize(&expanded).unwrap_or_else(|_| {
        if expanded.is_relative() {
            std::env::current_dir()
                .map(|cwd| cwd.join(&expanded))
                .unwrap_or_else(|_| expanded.clone())
        } else {
            expanded
        }
    });

    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        let meta = std::fs::metadata(&abs).context(IoSnafu { path: &abs })?;
        let mode = meta.permissions().mode();
        let group_other = mode & 0o077;
        ensure!(
            group_other == 0,
            InsecurePathSnafu {
                path: abs.clone(),
                reason: format!(
                    "identity file permissions 0{:o} are too open, expected 0600",
                    mode & 0o777
                ),
            }
        );
    }

    Ok(Some(abs))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::error::RockyError;

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
            std::fs::set_permissions(&insecure_key, std::fs::Permissions::from_mode(0o644))
                .unwrap();
            let res = canonicalize_identity(Some(&insecure_key));
            assert!(res.is_err());
            match res.unwrap_err() {
                RockyError::InsecurePath { .. } => {}
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
