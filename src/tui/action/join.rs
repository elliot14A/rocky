use std::path::Path;

use crate::commands::mesh::join::{self, JoinArgs};
use crate::error::Result;

pub async fn execute(fleet_root: &Path, env_name: &str, identity: Option<&Path>) -> Result<()> {
    let args = JoinArgs {
        env: Some(env_name.to_string()),
        env_flag: None,
        authkey: None,
        identity: identity.map(Path::to_path_buf),
        login_server: None,
        force_reauth: false,
    };

    join::run(args, Some(fleet_root)).await
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn test_join_missing_environment_fails() {
        let temp = tempfile::tempdir().unwrap();
        let result = execute(temp.path(), "nonexistent", None).await;
        assert!(result.is_err());
    }
}
