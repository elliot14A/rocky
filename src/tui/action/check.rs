use std::path::Path;

use snafu::ResultExt;

use crate::error::{CommandExecutionSnafu, ProcessSpawnSnafu, Result};

pub async fn execute(fleet_root: &Path) -> Result<Vec<String>> {
    let _guard = crate::commands::check::MockSecretGuard::new(fleet_root).await?;

    let output = tokio::process::Command::new("nix")
        .args(["flake", "check", "--no-build"])
        .current_dir(fleet_root)
        .kill_on_drop(true)
        .output()
        .await
        .context(ProcessSpawnSnafu {
            command: "nix flake check --no-build",
        })?;

    let stdout_str = String::from_utf8_lossy(&output.stdout);
    let stderr_str = String::from_utf8_lossy(&output.stderr);

    let mut lines = Vec::new();
    for l in stdout_str.lines().chain(stderr_str.lines()) {
        if !l.trim().is_empty() {
            lines.push(l.to_string());
        }
    }

    if output.status.success() {
        lines.push("all checks passed!".to_string());
        Ok(lines)
    } else {
        CommandExecutionSnafu {
            command: "nix flake check --no-build".to_string(),
            status: output.status.code().unwrap_or(1),
            stderr: stderr_str.to_string(),
            log_path: None,
        }
        .fail()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn test_check_non_fleet_directory_fails() {
        let temp = tempfile::tempdir().unwrap();
        let result = execute(temp.path()).await;
        assert!(result.is_err());
    }
}
