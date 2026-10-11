use std::path::Path;

use snafu::ResultExt;
use tempfile::tempdir;

use super::REPO;
use crate::error::{IoSnafu, NotFoundSnafu, Result, TargetNotEmptySnafu};
use crate::utils::validate::validate_name;
use crate::{fs, nix, process};

pub async fn fetch(template: &str, target: &Path, force: bool) -> Result<()> {
    validate_name("Template", template)?;

    if !force && !fs::is_dir_empty(target)? {
        return TargetNotEmptySnafu { path: target }.fail();
    }

    fs::ensure_dir(target)?;

    tracing::info!(template = %template, target = %target.display(), "fetching template");

    let temp_workspace = tempdir().context(IoSnafu {
        path: std::env::temp_dir(),
    })?;
    let temp_dir = temp_workspace.path();

    process::exec(
        "git",
        [
            std::ffi::OsStr::new("clone"),
            std::ffi::OsStr::new("--depth"),
            std::ffi::OsStr::new("1"),
            std::ffi::OsStr::new("--filter=blob:none"),
            std::ffi::OsStr::new("--sparse"),
            std::ffi::OsStr::new(REPO),
            temp_dir.as_os_str(),
        ],
        None,
    )
    .await?;

    let template_rel = format!("templates/{}", template);
    process::exec(
        "git",
        ["sparse-checkout", "set", "--", &template_rel],
        Some(temp_dir),
    )
    .await?;

    let src = temp_dir.join(&template_rel);
    if !src.exists() {
        return NotFoundSnafu {
            kind: "Template",
            name: template.to_string(),
            path: src,
        }
        .fail();
    }

    fs::copy_dir_all(&src, target)?;

    nix::fmt([target.as_os_str()]).await?;

    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn test_fetch_rejects_non_empty_target() {
        let dir = tempdir().unwrap();
        let target = dir.path();
        let file = target.join("existing.txt");
        fs::write_file(&file, "content").unwrap();

        let res = fetch("base", target, false).await;
        assert!(res.is_err());
    }

    #[tokio::test]
    async fn test_fetch_rejects_unknown_template() {
        let dir = tempdir().unwrap();
        let target = dir.path();

        let res = fetch("nonexistent-template-123", target, false).await;
        assert!(res.is_err());
    }

    #[tokio::test]
    async fn test_fetch_rejects_path_traversal_template() {
        let dir = tempdir().unwrap();
        let target = dir.path();

        let res = fetch("../.git", target, false).await;
        match res {
            Err(crate::error::RockyError::InvalidName { kind, name }) => {
                assert_eq!(kind, "Template");
                assert_eq!(name, "../.git");
            }
            other => panic!("expected InvalidName, got {:?}", other),
        }
    }

    #[test]
    fn test_is_valid_name_rules() {
        use crate::utils::is_valid_name;
        assert!(is_valid_name("base"));
        assert!(is_valid_name("my-template_1"));
        assert!(!is_valid_name(""));
        assert!(!is_valid_name("../evil"));
        assert!(!is_valid_name("--help"));
        assert!(!is_valid_name("temp/late"));
        assert!(!is_valid_name("temp\\late"));
    }
}
