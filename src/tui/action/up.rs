use std::path::Path;

use crate::commands::up::{self, UpArgs};
use crate::error::Result;

pub async fn execute(
    fleet_root: &Path,
    env_name: &str,
    target_host: Option<&str>,
    identity: Option<&Path>,
) -> Result<()> {
    let target = match target_host {
        Some(host) => vec![host.to_string()],
        None => Vec::new(),
    };
    let args = UpArgs {
        environment: env_name.to_string(),
        target,
        identity: identity.map(Path::to_path_buf),
        yes: false,
        non_interactive: false,
        concurrency: None,
    };
    up::run(args, Some(fleet_root)).await
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn test_up_missing_environment_fails() {
        let temp = tempfile::tempdir().unwrap();
        let result = execute(temp.path(), "nonexistent", None, None).await;
        assert!(result.is_err());
    }

    #[tokio::test]
    async fn test_up_targeted_host_missing_environment_fails() {
        let temp = tempfile::tempdir().unwrap();
        let result = execute(temp.path(), "nonexistent", Some("app"), None).await;
        assert!(result.is_err());
    }
}
