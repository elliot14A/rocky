use std::path::Path;

use snafu::OptionExt;

use crate::error::{Kind, NotFoundSnafu, Result};
use crate::tui::state::TuiState;

pub async fn show_selected_secret(fleet_root: &Path, state: &TuiState) -> Result<String> {
    let sec = state.selected_secret().context(NotFoundSnafu {
        kind: Kind::Secret,
        name: "selected secret".to_string(),
        path: fleet_root.to_path_buf(),
    })?;

    let env_dir = fleet_root.join("environments").join(&sec.environment);
    crate::crypto::secret::show::show_secret(&env_dir, &sec.name, state.identity.as_deref(), None)
        .await
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::tui::load::Fleet;

    #[tokio::test]
    async fn test_show_selected_secret_none_selected_fails() {
        let fleet = Fleet {
            environments: vec![],
            services: vec![],
        };
        let state = TuiState::new(fleet, None);
        let temp = tempfile::tempdir().unwrap();
        let res = show_selected_secret(temp.path(), &state).await;
        assert!(res.is_err());
    }
}
