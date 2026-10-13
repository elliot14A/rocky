use std::path::Path;

use snafu::ResultExt;
use tempfile::tempdir;

use super::REPO;
use crate::error::{IoSnafu, Kind, NotFoundSnafu, Result, TargetNotEmptySnafu};
use crate::utils::validate::validate_name;
use crate::{fs, nix, process};

async fn fetch_git_path(
    rel_path: &str,
    target: &Path,
    force: bool,
    kind: Kind,
    name: &str,
) -> Result<()> {
    if !force && !fs::is_dir_empty(target)? {
        return TargetNotEmptySnafu { path: target }.fail();
    }

    fs::ensure_dir(target)?;

    tracing::info!(path = %rel_path, target = %target.display(), "fetching template from origin");

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

    process::exec(
        "git",
        ["sparse-checkout", "set", "--", rel_path],
        Some(temp_dir),
    )
    .await?;

    let src = temp_dir.join(rel_path);
    if !src.exists() {
        return NotFoundSnafu {
            kind,
            name: name.to_string(),
            path: src,
        }
        .fail();
    }

    fs::copy_dir_all(&src, target)?;

    nix::fmt([target.as_os_str()]).await?;

    Ok(())
}

pub async fn fetch_template(template: &str, target: &Path, force: bool) -> Result<()> {
    validate_name(Kind::Template, template)?;
    fetch_git_path(
        &format!("templates/{}", template),
        target,
        force,
        Kind::Template,
        template,
    )
    .await
}

pub async fn fetch_provider(provider: &str, target: &Path) -> Result<()> {
    validate_name(Kind::Provider, provider)?;
    fetch_git_path(
        &format!("templates/providers/{}", provider),
        target,
        true,
        Kind::Provider,
        provider,
    )
    .await
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

        let res = fetch_template("base", target, false).await;
        assert!(res.is_err());
    }

    #[tokio::test]
    async fn test_fetch_rejects_unknown_template() {
        let dir = tempdir().unwrap();
        let target = dir.path();

        let res = fetch_template("nonexistent-template-123", target, false).await;
        assert!(res.is_err());
    }

    #[tokio::test]
    async fn test_fetch_rejects_path_traversal_template() {
        let dir = tempdir().unwrap();
        let target = dir.path();

        let res = fetch_template("../.git", target, false).await;
        match res {
            Err(crate::error::RockyError::InvalidName { kind, name }) => {
                assert_eq!(kind, Kind::Template);
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
