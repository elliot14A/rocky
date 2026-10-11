use std::path::Path;

use snafu::ResultExt;
use tap::prelude::*;

use super::dir::ensure_dir;
use crate::error::{CopySnafu, InsecurePathSnafu, IoSnafu, Result};

pub fn copy_file(src: &Path, dst: &Path) -> Result<()> {
    if std::fs::symlink_metadata(dst)
        .map(|m| m.is_symlink())
        .unwrap_or(false)
    {
        let _ = std::fs::remove_file(dst);
    }

    std::fs::copy(src, dst)
        .context(CopySnafu { src, dst })
        .tap_ok(|_| tracing::debug!(src = %src.display(), dst = %dst.display(), "copied file"))
        .map(|_| ())
}

pub fn copy_dir_all(src: &Path, dst: &Path) -> Result<()> {
    let src_canonical = src.canonicalize().context(IoSnafu { path: src })?;
    copy_dir_all_inner(src, dst, &src_canonical)
}

fn copy_dir_all_inner(src: &Path, dst: &Path, src_root: &Path) -> Result<()> {
    ensure_dir(dst)?;

    std::fs::read_dir(src)
        .context(IoSnafu { path: src })?
        .try_for_each(|entry| {
            let entry = entry.context(IoSnafu { path: src })?;
            let file_type = entry.file_type().context(IoSnafu { path: src })?;
            let entry_path = entry.path();
            let dest_path = dst.join(entry.file_name());

            if file_type.is_symlink() {
                let link_target =
                    std::fs::read_link(&entry_path).context(IoSnafu { path: &entry_path })?;
                let resolved = if link_target.is_absolute() {
                    link_target.clone()
                } else {
                    entry_path.parent().unwrap_or(src).join(&link_target)
                };

                let canonical = resolved
                    .canonicalize()
                    .context(IoSnafu { path: &resolved })?;
                if !canonical.starts_with(src_root) {
                    return InsecurePathSnafu {
                        path: entry_path,
                        reason: "symlink target escapes source directory",
                    }
                    .fail();
                }

                if std::fs::symlink_metadata(&dest_path).is_ok() {
                    let _ = std::fs::remove_file(&dest_path);
                }

                #[cfg(unix)]
                {
                    std::os::unix::fs::symlink(&link_target, &dest_path)
                        .context(IoSnafu { path: &dest_path })?;
                }

                #[cfg(not(unix))]
                {
                    if canonical.is_dir() {
                        copy_dir_all_inner(&canonical, &dest_path, src_root)?;
                    } else {
                        copy_file(&canonical, &dest_path)?;
                    }
                }

                Ok(())
            } else if entry_path.is_dir() {
                copy_dir_all_inner(&entry_path, &dest_path, src_root)
            } else {
                copy_file(&entry_path, &dest_path)
            }
        })
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::tempdir;

    #[test]
    fn test_copy_dir_all_recursive() {
        let src_dir = tempdir().unwrap();
        let dst_dir = tempdir().unwrap();

        let nested = src_dir.path().join("nested");
        ensure_dir(&nested).unwrap();

        let file1 = src_dir.path().join("root.txt");
        let file2 = nested.join("leaf.txt");

        std::fs::write(&file1, "root content").unwrap();
        std::fs::write(&file2, "leaf content").unwrap();

        copy_dir_all(src_dir.path(), dst_dir.path()).unwrap();

        assert_eq!(
            std::fs::read_to_string(dst_dir.path().join("root.txt")).unwrap(),
            "root content"
        );
        assert_eq!(
            std::fs::read_to_string(dst_dir.path().join("nested/leaf.txt")).unwrap(),
            "leaf content"
        );
    }

    #[cfg(unix)]
    #[test]
    fn test_copy_dir_rejects_escaping_symlink() {
        let src_dir = tempdir().unwrap();
        let dst_dir = tempdir().unwrap();
        let secret = tempdir().unwrap();
        let secret_file = secret.path().join("secret.txt");
        std::fs::write(&secret_file, "top-secret").unwrap();

        let link = src_dir.path().join("leak.txt");
        std::os::unix::fs::symlink(&secret_file, &link).unwrap();

        let res = copy_dir_all(src_dir.path(), dst_dir.path());
        assert!(res.is_err());
    }

    #[cfg(unix)]
    #[test]
    fn test_copy_file_overwrites_symlink_safely() {
        let src_dir = tempdir().unwrap();
        let dst_dir = tempdir().unwrap();
        let decoy = tempdir().unwrap();
        let target_victim = decoy.path().join("victim.txt");
        std::fs::write(&target_victim, "original").unwrap();

        let src_file = src_dir.path().join("src.txt");
        std::fs::write(&src_file, "new-content").unwrap();

        let dst_link = dst_dir.path().join("dst.txt");
        std::os::unix::fs::symlink(&target_victim, &dst_link).unwrap();

        copy_file(&src_file, &dst_link).unwrap();

        assert_eq!(std::fs::read_to_string(&target_victim).unwrap(), "original");
        assert_eq!(std::fs::read_to_string(&dst_link).unwrap(), "new-content");
    }
}
