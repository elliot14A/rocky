use std::collections::HashSet;
use std::path::PathBuf;

use super::load::{Environment, Fleet, Host};

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum TreeItem {
    Environment {
        name: String,
        mesh_connected: bool,
        expanded: bool,
    },
    Host {
        name: String,
        env_name: String,
    },
}

#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub enum ActivePopup {
    #[default]
    None,
    ConfirmJoinMesh {
        env_name: String,
    },
    NewEntityMenu,
    PromptInput {
        title: String,
        value: String,
    },
    Error(String),
}

#[derive(Debug, Clone)]
pub struct TuiState {
    pub fleet: Fleet,
    pub selected_index: usize,
    pub expanded_envs: HashSet<String>,
    pub identity: Option<PathBuf>,
    pub active_popup: ActivePopup,
    pub status_message: Option<String>,
    items: Vec<TreeItem>,
}

impl TuiState {
    pub fn new(fleet: Fleet, identity: Option<PathBuf>) -> Self {
        let mut expanded_envs = HashSet::new();
        for env in &fleet.environments {
            expanded_envs.insert(env.name.clone());
        }

        let mut state = Self {
            fleet,
            selected_index: 0,
            expanded_envs,
            identity,
            active_popup: ActivePopup::None,
            status_message: None,
            items: Vec::new(),
        };

        state.rebuild_items();
        state
    }

    pub fn rebuild_items(&mut self) {
        let mut items = Vec::new();

        for env in &self.fleet.environments {
            let expanded = self.expanded_envs.contains(&env.name);
            items.push(TreeItem::Environment {
                name: env.name.clone(),
                mesh_connected: env.mesh_connected,
                expanded,
            });

            if expanded {
                for host in &env.hosts {
                    items.push(TreeItem::Host {
                        name: host.name.clone(),
                        env_name: env.name.clone(),
                    });
                }
            }
        }

        self.items = items;
        if self.items.is_empty() {
            self.selected_index = 0;
        } else if self.selected_index >= self.items.len() {
            self.selected_index = self.items.len() - 1;
        }
    }

    pub fn items(&self) -> &[TreeItem] {
        &self.items
    }

    pub fn selected_item(&self) -> Option<&TreeItem> {
        self.items.get(self.selected_index)
    }

    pub fn move_down(&mut self) {
        if self.items.is_empty() {
            return;
        }
        if self.selected_index + 1 >= self.items.len() {
            self.selected_index = 0;
        } else {
            self.selected_index += 1;
        }
    }

    pub fn move_up(&mut self) {
        if self.items.is_empty() {
            return;
        }
        if self.selected_index == 0 {
            self.selected_index = self.items.len() - 1;
        } else {
            self.selected_index -= 1;
        }
    }

    pub fn toggle_expand(&mut self) {
        if let Some(TreeItem::Environment { name, .. }) = self.selected_item() {
            let env_name = name.clone();
            if self.expanded_envs.contains(&env_name) {
                self.expanded_envs.remove(&env_name);
            } else {
                self.expanded_envs.insert(env_name);
            }
            self.rebuild_items();
        }
    }

    pub fn selected_env(&self) -> Option<&Environment> {
        match self.selected_item()? {
            TreeItem::Environment { name, .. } => {
                self.fleet.environments.iter().find(|e| &e.name == name)
            }
            TreeItem::Host { env_name, .. } => {
                self.fleet.environments.iter().find(|e| &e.name == env_name)
            }
        }
    }

    pub fn selected_host(&self) -> Option<&Host> {
        match self.selected_item()? {
            TreeItem::Host { name, env_name } => {
                let env = self.fleet.environments.iter().find(|e| &e.name == env_name)?;
                env.hosts.iter().find(|h| &h.name == name)
            }
            TreeItem::Environment { .. } => None,
        }
    }

    pub fn reload(&mut self, fleet: Fleet) {
        self.fleet = fleet;
        self.rebuild_items();
    }

    pub fn set_status(&mut self, message: impl Into<String>) {
        self.status_message = Some(message.into());
    }

    pub fn clear_status(&mut self) {
        self.status_message = None;
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn sample_fleet() -> Fleet {
        Fleet {
            environments: vec![Environment {
                name: "prod".to_string(),
                hosts: vec![
                    Host {
                        name: "app".to_string(),
                        environment: "prod".to_string(),
                        services: vec!["gotrue".to_string()],
                        magic_dns: "prod-app".to_string(),
                        has_disko: true,
                        has_hardware: true,
                        has_key: true,
                    },
                    Host {
                        name: "db".to_string(),
                        environment: "prod".to_string(),
                        services: vec!["postgres".to_string()],
                        magic_dns: "prod-db".to_string(),
                        has_disko: true,
                        has_hardware: true,
                        has_key: true,
                    },
                    Host {
                        name: "edge".to_string(),
                        environment: "prod".to_string(),
                        services: vec!["caddy".to_string()],
                        magic_dns: "prod-edge".to_string(),
                        has_disko: true,
                        has_hardware: true,
                        has_key: true,
                    },
                ],
                secrets: vec![],
                mesh_connected: true,
            }],
            services: vec![],
        }
    }

    #[test]
    fn test_tui_state_navigation_and_wrapping() {
        let fleet = sample_fleet();
        let mut state = TuiState::new(fleet, Some(PathBuf::from("/tmp/id_ed25519")));

        assert_eq!(state.items().len(), 4);
        assert_eq!(state.selected_index, 0);
        assert_eq!(state.selected_env().map(|e| e.name.as_str()), Some("prod"));
        assert!(state.selected_host().is_none());

        state.move_down();
        assert_eq!(state.selected_index, 1);
        assert_eq!(state.selected_env().map(|e| e.name.as_str()), Some("prod"));
        assert_eq!(state.selected_host().map(|h| h.name.as_str()), Some("app"));

        state.move_down();
        assert_eq!(state.selected_index, 2);
        assert_eq!(state.selected_host().map(|h| h.name.as_str()), Some("db"));

        state.move_down();
        assert_eq!(state.selected_index, 3);
        assert_eq!(state.selected_host().map(|h| h.name.as_str()), Some("edge"));

        state.move_down();
        assert_eq!(state.selected_index, 0);

        state.move_up();
        assert_eq!(state.selected_index, 3);

        assert_eq!(
            state.identity.as_deref(),
            Some(Path::new("/tmp/id_ed25519"))
        );
    }

    #[test]
    fn test_tui_state_toggle_collapse() {
        let fleet = sample_fleet();
        let mut state = TuiState::new(fleet, None);

        assert_eq!(state.items().len(), 4);
        state.toggle_expand();

        assert_eq!(state.items().len(), 1);
        assert_eq!(state.selected_index, 0);
        assert_eq!(state.selected_env().map(|e| e.name.as_str()), Some("prod"));
        assert!(state.selected_host().is_none());

        state.move_down();
        assert_eq!(state.selected_index, 0);

        state.toggle_expand();
        assert_eq!(state.items().len(), 4);
    }
}
