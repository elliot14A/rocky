use std::path::Path;

use snafu::ResultExt;
use tap::prelude::*;

use super::dir::ensure_dir;
use crate::error::{CopySnafu, IoSnafu, Result};

pub fn copy_file(src: &Path, dst: &Path) -> Result<()> {
    std::fs::copy(src, dst)
        .context(CopySnafu { src, dst })
        .tap_ok(|_| tracing::debug!(src = %src.display(), dst = %dst.display(), "copied file"))
        .map(|_| ())
}

pub fn copy_dir_all(src: &Path, dst: &Path) -> Result<()> {
    ensure_dir(dst)?;

    std::fs::read_dir(src)
        .context(IoSnafu { path: src })?
        .collect::<std::result::Result<Vec<_>, std::io::Error>>()
        .context(IoSnafu { path: src })?
        .into_iter()
        .try_for_each(|entry| {
            let entry_path = entry.path();
            let dest_path = dst.join(entry.file_name());

            entry
                .file_type()
                .context(IoSnafu { path: &entry_path })?
                .is_dir()
                .pipe(|is_dir| {
                    if is_dir {
                        copy_dir_all(&entry_path, &dest_path)
                    } else {
                        copy_file(&entry_path, &dest_path)
                    }
                })
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
}
