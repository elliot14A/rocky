use std::fs;
use std::path::Path;

use snafu::ResultExt;
use tap::prelude::*;

use crate::error::{CopySnafu, IoSnafu, NotADirectorySnafu, Result};

pub fn write_file(path: &Path, content: &str) -> Result<()> {
    fs::write(path, content)
        .context(IoSnafu { path })
        .tap_ok(|_| tracing::debug!(path = %path.display(), "wrote file"))
}

pub fn ensure_dir(path: &Path) -> Result<()> {
    fs::create_dir_all(path)
        .context(IoSnafu { path })
        .tap_ok(|_| tracing::debug!(path = %path.display(), "ensured directory"))
}

pub fn is_dir_empty(path: &Path) -> Result<bool> {
    if !path.exists() {
        return Ok(true);
    }

    if !path.is_dir() {
        return NotADirectorySnafu { path }.fail();
    }

    fs::read_dir(path)
        .context(IoSnafu { path })?
        .next()
        .is_none()
        .pipe(Ok)
}

pub fn copy_file(src: &Path, dst: &Path) -> Result<()> {
    fs::copy(src, dst)
        .context(CopySnafu { src, dst })
        .tap_ok(|_| tracing::debug!(src = %src.display(), dst = %dst.display(), "copied file"))
        .map(|_| ())
}

pub fn copy_dir_all(src: &Path, dst: &Path) -> Result<()> {
    ensure_dir(dst)?;

    fs::read_dir(src)
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
    fn test_is_dir_empty_non_existent() {
        let dir = tempdir().unwrap();
        let missing = dir.path().join("missing");
        assert!(is_dir_empty(&missing).unwrap());
    }

    #[test]
    fn test_is_dir_empty_empty_directory() {
        let dir = tempdir().unwrap();
        assert!(is_dir_empty(dir.path()).unwrap());
    }

    #[test]
    fn test_is_dir_empty_with_file() {
        let dir = tempdir().unwrap();
        let file = dir.path().join("sample.txt");
        write_file(&file, "content").unwrap();
        assert!(!is_dir_empty(dir.path()).unwrap());
    }

    #[test]
    fn test_is_dir_empty_errors_on_plain_file() {
        let dir = tempdir().unwrap();
        let file = dir.path().join("file.txt");
        write_file(&file, "data").unwrap();
        assert!(is_dir_empty(&file).is_err());
    }

    #[test]
    fn test_copy_dir_all_recursive() {
        let src_dir = tempdir().unwrap();
        let dst_dir = tempdir().unwrap();

        let nested = src_dir.path().join("nested");
        ensure_dir(&nested).unwrap();

        let file1 = src_dir.path().join("root.txt");
        let file2 = nested.join("leaf.txt");

        write_file(&file1, "root content").unwrap();
        write_file(&file2, "leaf content").unwrap();

        copy_dir_all(src_dir.path(), dst_dir.path()).unwrap();

        assert_eq!(
            fs::read_to_string(dst_dir.path().join("root.txt")).unwrap(),
            "root content"
        );
        assert_eq!(
            fs::read_to_string(dst_dir.path().join("nested/leaf.txt")).unwrap(),
            "leaf content"
        );
    }
}
