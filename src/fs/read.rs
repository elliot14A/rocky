use std::path::Path;

use snafu::ResultExt;
use tap::prelude::*;

use crate::error::{IoSnafu, Result};

pub fn read_file(path: &Path) -> Result<String> {
    std::fs::read_to_string(path)
        .context(IoSnafu { path })
        .tap_ok(|_| tracing::debug!(path = %path.display(), "read file"))
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::tempdir;

    #[test]
    fn test_read_file_success() {
        let dir = tempdir().unwrap();
        let file = dir.path().join("test.txt");
        std::fs::write(&file, "hello").unwrap();

        let content = read_file(&file).unwrap();
        assert_eq!(content, "hello");
    }

    #[test]
    fn test_read_file_not_found() {
        let dir = tempdir().unwrap();
        let file = dir.path().join("nonexistent.txt");

        assert!(read_file(&file).is_err());
    }
}
