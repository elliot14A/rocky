use std::path::{Path, PathBuf};

/// RAII guard that rolls back filesystem changes if dropped before being committed.
#[derive(Debug, Default)]
pub struct Rollback {
    dirs_to_remove: Vec<PathBuf>,
    files_to_remove: Vec<PathBuf>,
    files_to_restore: Vec<(PathBuf, String)>,
    committed: bool,
}

impl Rollback {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn remove_dir_on_failure(&mut self, path: impl AsRef<Path>) {
        self.dirs_to_remove.push(path.as_ref().to_path_buf());
    }

    pub fn remove_file_on_failure(&mut self, path: impl AsRef<Path>) {
        self.files_to_remove.push(path.as_ref().to_path_buf());
    }

    pub fn restore_file_on_failure(
        &mut self,
        path: impl AsRef<Path>,
        original_content: impl Into<String>,
    ) {
        self.files_to_restore
            .push((path.as_ref().to_path_buf(), original_content.into()));
    }

    pub fn commit(mut self) {
        self.committed = true;
    }
}

impl Drop for Rollback {
    fn drop(&mut self) {
        if !self.committed {
            for (path, content) in self.files_to_restore.iter().rev() {
                if let Err(err) = std::fs::write(path, content) {
                    tracing::warn!(path = %path.display(), error = %err, "failed to restore file during rollback");
                }
            }
            for file in self.files_to_remove.iter().rev() {
                if let Err(err) = std::fs::remove_file(file) {
                    tracing::warn!(path = %file.display(), error = %err, "failed to remove file during rollback");
                }
            }
            for dir in self.dirs_to_remove.iter().rev() {
                if let Err(err) = std::fs::remove_dir_all(dir) {
                    tracing::warn!(path = %dir.display(), error = %err, "failed to remove directory during rollback");
                }
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::tempdir;

    #[test]
    fn test_rollback_removes_dir_on_drop() {
        let dir = tempdir().unwrap();
        let target_dir = dir.path().join("scaffold");
        std::fs::create_dir_all(&target_dir).unwrap();
        assert!(target_dir.exists());

        {
            let mut rollback = Rollback::new();
            rollback.remove_dir_on_failure(&target_dir);
            // Dropped without calling commit()
        }

        assert!(!target_dir.exists());
    }

    #[test]
    fn test_rollback_restores_file_on_drop() {
        let dir = tempdir().unwrap();
        let file_path = dir.path().join("config.nix");
        std::fs::write(&file_path, "original content").unwrap();

        {
            let mut rollback = Rollback::new();
            rollback.restore_file_on_failure(&file_path, "original content");

            std::fs::write(&file_path, "modified corrupted content").unwrap();
            assert_eq!(
                std::fs::read_to_string(&file_path).unwrap(),
                "modified corrupted content"
            );
            // Dropped without calling commit()
        }

        assert_eq!(
            std::fs::read_to_string(&file_path).unwrap(),
            "original content"
        );
    }

    #[test]
    fn test_rollback_preserves_changes_when_committed() {
        let dir = tempdir().unwrap();
        let target_dir = dir.path().join("scaffold");
        let file_path = target_dir.join("config.nix");
        std::fs::create_dir_all(&target_dir).unwrap();
        std::fs::write(&file_path, "initial").unwrap();

        {
            let mut rollback = Rollback::new();
            rollback.remove_dir_on_failure(&target_dir);
            rollback.restore_file_on_failure(&file_path, "initial");

            std::fs::write(&file_path, "new content").unwrap();
            rollback.commit();
        }

        assert!(target_dir.exists());
        assert_eq!(std::fs::read_to_string(&file_path).unwrap(), "new content");
    }
}
