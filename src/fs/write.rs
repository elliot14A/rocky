use std::path::Path;

use snafu::ResultExt;
use tap::prelude::*;

use super::dir::ensure_dir;
use crate::error::{IoSnafu, Result};

pub fn write_file(path: &Path, content: &str) -> Result<()> {
    if let Some(parent) = path.parent() {
        ensure_dir(parent)?;
    }

    std::fs::write(path, content)
        .context(IoSnafu { path })
        .tap_ok(|_| tracing::debug!(path = %path.display(), "wrote file"))
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
}
