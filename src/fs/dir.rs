use std::path::Path;

use snafu::ResultExt;
use tap::prelude::*;

use crate::error::{IoSnafu, NotADirectorySnafu, Result};

pub fn ensure_dir(path: &Path) -> Result<()> {
    std::fs::create_dir_all(path)
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

    std::fs::read_dir(path)
        .context(IoSnafu { path })?
        .next()
        .is_none()
        .pipe(Ok)
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
        std::fs::write(&file, "content").unwrap();
        assert!(!is_dir_empty(dir.path()).unwrap());
    }

    #[test]
    fn test_is_dir_empty_errors_on_plain_file() {
        let dir = tempdir().unwrap();
        let file = dir.path().join("file.txt");
        std::fs::write(&file, "data").unwrap();
        assert!(is_dir_empty(&file).is_err());
    }
}
