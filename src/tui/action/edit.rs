use std::path::{Path, PathBuf};

use snafu::OptionExt;

use crate::error::{Kind, NotFoundSnafu, Result};
use crate::process::editor::run_editor;
use crate::tui::state::{FocusPanel, TreeItem, TuiState};

pub fn resolve_edit_target(fleet_root: &Path, state: &TuiState) -> Option<(PathBuf, Kind)> {
    match state.focus {
        FocusPanel::Services => {
            let svc = state.selected_service()?;
            let path = fleet_root
                .join("services")
                .join(format!("{}.nix", svc.name));
            Some((path, Kind::Service))
        }
        FocusPanel::Secrets => {
            let sec = state.selected_secret()?;
            let path = fleet_root
                .join("environments")
                .join(&sec.environment)
                .join("secrets")
                .join(&sec.name);
            Some((path, Kind::Secret))
        }
        _ => match state.selected_tree_item()? {
            TreeItem::Environment { name, .. } => {
                let path = fleet_root.join("environments").join(name).join("fleet.nix");
                Some((path, Kind::Environment))
            }
            TreeItem::Host { name, env_name, .. } => {
                let path = fleet_root
                    .join("environments")
                    .join(env_name)
                    .join("hosts")
                    .join(name)
                    .join("default.nix");
                Some((path, Kind::Host))
            }
            TreeItem::HostService { name, .. } => {
                let path = fleet_root.join("services").join(format!("{name}.nix"));
                Some((path, Kind::Service))
            }
        },
    }
}

pub async fn execute(fleet_root: &Path, state: &TuiState) -> Result<bool> {
    if let crate::tui::state::ActivePopup::SecretViewer(viewer) = &state.active_popup {
        let env_dir = fleet_root.join("environments").join(&viewer.environment);
        return crate::crypto::secret::edit::edit_secret(
            &env_dir,
            &viewer.name,
            state.identity.as_deref(),
        )
        .await;
    }

    if state.focus == FocusPanel::Secrets {
        let sec = state.selected_secret().context(NotFoundSnafu {
            kind: Kind::Secret,
            name: "selected secret".to_string(),
            path: fleet_root.to_path_buf(),
        })?;
        let env_dir = fleet_root.join("environments").join(&sec.environment);
        return crate::crypto::secret::edit::edit_secret(
            &env_dir,
            &sec.name,
            state.identity.as_deref(),
        )
        .await;
    }

    let (target_path, kind) = resolve_edit_target(fleet_root, state).context(NotFoundSnafu {
        kind: Kind::Fleet,
        name: "selected item".to_string(),
        path: fleet_root.to_path_buf(),
    })?;

    run_editor(&target_path, kind).await
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::tui::load::{Environment, Fleet, Host, Secret, Service};

    fn test_fleet() -> Fleet {
        Fleet {
            environments: vec![Environment {
                name: "prod".to_string(),
                domain: "prod.rocky.internal".to_string(),
                hosts: vec![Host {
                    name: "app".to_string(),
                    environment: "prod".to_string(),
                    services: vec![],
                    magic_dns: "prod-app".to_string(),
                    has_disko: true,
                    has_hardware: true,
                    has_key: true,
                }],
                secrets: vec![Secret {
                    name: "jwt.age".to_string(),
                    environment: "prod".to_string(),
                    recipients: "all hosts".to_string(),
                    services: vec![],
                }],
                mesh_connected: true,
            }],
            services: vec![Service {
                name: "caddy".to_string(),
                kind: "systemd".to_string(),
                target: "caddy.service".to_string(),
                replicas: 1,
                base_port: None,
                environments: vec!["prod".to_string()],
                hosts: vec!["prod:app".to_string()],
                secrets: vec![],
            }],
        }
    }

    #[test]
    fn test_resolve_edit_target_host() {
        let mut state = TuiState::new(test_fleet(), None);
        state.env_host_index = 1;
        let root = Path::new("/fleet");

        let resolved = resolve_edit_target(root, &state);
        assert_eq!(
            resolved,
            Some((
                PathBuf::from("/fleet/environments/prod/hosts/app/default.nix"),
                Kind::Host
            ))
        );
    }

    #[test]
    fn test_resolve_edit_target_service() {
        let mut state = TuiState::new(test_fleet(), None);
        state.focus = FocusPanel::Services;
        state.service_index = 0;
        let root = Path::new("/fleet");

        let resolved = resolve_edit_target(root, &state);
        assert_eq!(
            resolved,
            Some((PathBuf::from("/fleet/services/caddy.nix"), Kind::Service))
        );
    }
}
