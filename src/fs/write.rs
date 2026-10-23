use std::io::Write;
use std::path::Path;

use snafu::ResultExt;
use tap::prelude::*;

use super::dir::ensure_dir;
use crate::error::{IoSnafu, Result, RockyError};

fn atomic_write(path: &Path, bytes: &[u8]) -> Result<()> {
    let parent = path.parent().unwrap_or_else(|| Path::new("."));
    ensure_dir(parent)?;

    if let Ok(meta) = std::fs::symlink_metadata(path) {
        if meta.file_type().is_symlink() {
            let _ = std::fs::remove_file(path);
        }
    }

    let mut temp = tempfile::Builder::new()
        .prefix(".rocky-tmp-")
        .tempfile_in(parent)
        .context(IoSnafu { path: parent })?;

    temp.write_all(bytes).context(IoSnafu { path })?;
    temp.flush().context(IoSnafu { path })?;

    temp.persist(path).map_err(|e| RockyError::Io {
        path: path.to_path_buf(),
        source: e.error,
    })?;

    Ok(())
}

pub fn write_file(path: &Path, content: &str) -> Result<()> {
    atomic_write(path, content.as_bytes())
        .tap_ok(|_| tracing::debug!(path = %path.display(), "wrote file"))
}

pub fn write_file_bytes(path: &Path, content: &[u8]) -> Result<()> {
    atomic_write(path, content)
        .tap_ok(|_| tracing::debug!(path = %path.display(), "wrote file bytes"))
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::tempdir;

    #[test]
    fn test_write_file_creates_nested_parent_dirs() {
        let dir = tempdir().unwrap();
        let target = dir.path().join("nested").join("sub").join("file.txt");

        assert!(write_file(&target, "content").is_ok());
        assert_eq!(std::fs::read_to_string(&target).unwrap(), "content");
    }

    #[test]
    fn test_write_file_removes_existing_symlink_safely() {
        let dir = tempdir().unwrap();
        let sensitive = dir.path().join("sensitive.txt");
        std::fs::write(&sensitive, "original-sensitive").unwrap();

        let link = dir.path().join("link.txt");
        #[cfg(unix)]
        std::os::unix::fs::symlink(&sensitive, &link).unwrap();

        #[cfg(unix)]
        {
            assert!(write_file(&link, "new-content").is_ok());
            // Link is replaced with regular file, target is untouched
            assert_eq!(
                std::fs::read_to_string(&sensitive).unwrap(),
                "original-sensitive"
            );
            assert_eq!(std::fs::read_to_string(&link).unwrap(), "new-content");
            assert!(!link.is_symlink());
        }
    }
}
