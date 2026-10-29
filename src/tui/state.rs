use std::cell::Cell;
use std::collections::HashSet;
use std::path::PathBuf;

use super::load::{Environment, Fleet, Host, Secret, Service};
pub use crate::templates::service::ServiceKind;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum FocusPanel {
    Details = 0,
    Status = 1,
    #[default]
    Environments = 2,
    Services = 3,
    Secrets = 4,
    Log = 5,
}

impl FocusPanel {
    pub fn next(self) -> Self {
        match self {
            Self::Details => Self::Status,
            Self::Status => Self::Environments,
            Self::Environments => Self::Services,
            Self::Services => Self::Secrets,
            Self::Secrets => Self::Log,
            Self::Log => Self::Details,
        }
    }

    pub fn prev(self) -> Self {
        match self {
            Self::Details => Self::Log,
            Self::Status => Self::Details,
            Self::Environments => Self::Status,
            Self::Services => Self::Environments,
            Self::Secrets => Self::Services,
            Self::Log => Self::Secrets,
        }
    }

    pub fn from_digit(digit: char) -> Option<Self> {
        match digit {
            '0' => Some(Self::Details),
            '1' => Some(Self::Status),
            '2' => Some(Self::Environments),
            '3' => Some(Self::Services),
            '4' => Some(Self::Secrets),
            '5' => Some(Self::Log),
            _ => None,
        }
    }
}

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
        has_services: bool,
        expanded: bool,
    },
    HostService {
        name: String,
        host_name: String,
        env_name: String,
        is_last: bool,
    },
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct HostForm {
    pub name: String,
    pub environment: String,
    pub provider: String,
    pub active_field: usize,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ServiceForm {
    pub name: String,
    pub kind: ServiceKind,
    pub host: Option<String>,
    pub port: String,
    pub replicas: u32,
    pub active_field: usize,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SecretForm {
    pub name: String,
    pub environment: String,
    pub all_hosts: bool,
    pub active_field: usize,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct EnvForm {
    pub name: String,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct HelpModalState {
    pub filter: String,
    pub selected_index: usize,
    pub scroll: usize,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SecretViewerState {
    pub name: String,
    pub environment: String,
    pub content: String,
    pub scroll: usize,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum PendingSecretAction {
    View,
    Edit,
    None,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct IdentityModalState {
    pub input: String,
    pub pending: PendingSecretAction,
    pub error: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub enum ActivePopup {
    #[default]
    None,
    HelpModal(HelpModalState),
    SecretViewer(SecretViewerState),
    IdentityModal(IdentityModalState),
    ConfirmJoinMesh {
        env_name: String,
    },
    NewEntityMenu,
    HostModal(HostForm),
    ServiceModal(ServiceForm),
    SecretModal(SecretForm),
    EnvModal(EnvForm),
    Error(String),
}

#[derive(Debug, Clone)]
pub struct TuiState {
    pub fleet: Fleet,
    pub focus: FocusPanel,
    pub env_host_index: usize,
    pub env_scroll: Cell<usize>,
    pub service_index: usize,
    pub service_scroll: Cell<usize>,
    pub secret_index: usize,
    pub secret_scroll: Cell<usize>,
    pub log_scroll: usize,
    pub logs_maximized: bool,
    pub expanded_envs: HashSet<String>,
    pub expanded_hosts: HashSet<String>,
    pub identity: Option<PathBuf>,
    pub active_popup: ActivePopup,
    pub command_log: Vec<String>,
    items: Vec<TreeItem>,
}

pub fn adjust_scroll(index: usize, scroll: &mut usize, max_rows: usize) {
    if max_rows == 0 {
        *scroll = 0;
        return;
    }
    if index < *scroll {
        *scroll = index;
    } else if index >= *scroll + max_rows {
        *scroll = index - max_rows + 1;
    }
}

impl TuiState {
    pub fn new(fleet: Fleet, identity: Option<PathBuf>) -> Self {
        let mut expanded_envs = HashSet::new();
        let expanded_hosts = HashSet::new();
        if let Some(first) = fleet.environments.first() {
            expanded_envs.insert(first.name.clone());
        }

        let mut state = Self {
            fleet,
            focus: FocusPanel::Environments,
            env_host_index: 0,
            env_scroll: Cell::new(0),
            service_index: 0,
            service_scroll: Cell::new(0),
            secret_index: 0,
            secret_scroll: Cell::new(0),
            log_scroll: 0,
            logs_maximized: false,
            expanded_envs,
            expanded_hosts,
            identity,
            active_popup: ActivePopup::None,
            command_log: Vec::new(),
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
                    let host_key = format!("{}:{}", env.name, host.name);
                    let host_expanded = self.expanded_hosts.contains(&host_key);
                    let has_services = !host.services.is_empty();

                    items.push(TreeItem::Host {
                        name: host.name.clone(),
                        env_name: env.name.clone(),
                        has_services,
                        expanded: host_expanded,
                    });

                    if host_expanded {
                        let total_svcs = host.services.len();
                        for (idx, svc_name) in host.services.iter().enumerate() {
                            items.push(TreeItem::HostService {
                                name: svc_name.clone(),
                                host_name: host.name.clone(),
                                env_name: env.name.clone(),
                                is_last: idx + 1 == total_svcs,
                            });
                        }
                    }
                }
            }
        }

        self.items = items;
        if self.items.is_empty() {
            self.env_host_index = 0;
        } else if self.env_host_index >= self.items.len() {
            self.env_host_index = self.items.len() - 1;
        }

        if self.fleet.services.is_empty() {
            self.service_index = 0;
        } else if self.service_index >= self.fleet.services.len() {
            self.service_index = self.fleet.services.len() - 1;
        }

        let secrets_count = self.visible_secrets().len();
        if secrets_count == 0 {
            self.secret_index = 0;
        } else if self.secret_index >= secrets_count {
            self.secret_index = secrets_count - 1;
        }
    }

    pub fn tree_items(&self) -> &[TreeItem] {
        &self.items
    }

    pub fn selected_tree_item(&self) -> Option<&TreeItem> {
        self.items.get(self.env_host_index)
    }

    pub fn selected_env(&self) -> Option<&Environment> {
        match self.selected_tree_item()? {
            TreeItem::Environment { name, .. } => {
                self.fleet.environments.iter().find(|e| &e.name == name)
            }
            TreeItem::Host { env_name, .. } => {
                self.fleet.environments.iter().find(|e| &e.name == env_name)
            }
            TreeItem::HostService { env_name, .. } => {
                self.fleet.environments.iter().find(|e| &e.name == env_name)
            }
        }
    }

    pub fn selected_host(&self) -> Option<&Host> {
        match self.selected_tree_item()? {
            TreeItem::Host { name, env_name, .. } => {
                let env = self
                    .fleet
                    .environments
                    .iter()
                    .find(|e| &e.name == env_name)?;
                env.hosts.iter().find(|h| &h.name == name)
            }
            TreeItem::HostService {
                host_name,
                env_name,
                ..
            } => {
                let env = self
                    .fleet
                    .environments
                    .iter()
                    .find(|e| &e.name == env_name)?;
                env.hosts.iter().find(|h| &h.name == host_name)
            }
            TreeItem::Environment { .. } => None,
        }
    }

    pub fn selected_service(&self) -> Option<&Service> {
        if self.focus == FocusPanel::Environments {
            if let Some(TreeItem::HostService { name, .. }) = self.selected_tree_item() {
                return self.fleet.services.iter().find(|s| &s.name == name);
            }
        }
        self.fleet.services.get(self.service_index)
    }

    pub fn visible_secrets(&self) -> &[Secret] {
        if let Some(env) = self.selected_env() {
            &env.secrets
        } else if let Some(first_env) = self.fleet.environments.first() {
            &first_env.secrets
        } else {
            &[]
        }
    }

    pub fn selected_secret(&self) -> Option<&Secret> {
        self.visible_secrets().get(self.secret_index)
    }

    pub fn move_down(&mut self) {
        match self.focus {
            FocusPanel::Environments if !self.items.is_empty() => {
                self.env_host_index = (self.env_host_index + 1) % self.items.len();
            }
            FocusPanel::Services if !self.fleet.services.is_empty() => {
                self.service_index = (self.service_index + 1) % self.fleet.services.len();
            }
            FocusPanel::Secrets => {
                let total = self.visible_secrets().len();
                if total > 0 {
                    self.secret_index = (self.secret_index + 1) % total;
                }
            }
            FocusPanel::Log => {
                self.scroll_log_down(1);
            }
            _ => {}
        }
    }

    pub fn move_up(&mut self) {
        match self.focus {
            FocusPanel::Environments if !self.items.is_empty() => {
                if self.env_host_index == 0 {
                    self.env_host_index = self.items.len() - 1;
                } else {
                    self.env_host_index -= 1;
                }
            }
            FocusPanel::Services if !self.fleet.services.is_empty() => {
                if self.service_index == 0 {
                    self.service_index = self.fleet.services.len() - 1;
                } else {
                    self.service_index -= 1;
                }
            }
            FocusPanel::Secrets => {
                let total = self.visible_secrets().len();
                if total > 0 {
                    if self.secret_index == 0 {
                        self.secret_index = total - 1;
                    } else {
                        self.secret_index -= 1;
                    }
                }
            }
            FocusPanel::Log => {
                self.scroll_log_up(1);
            }
            _ => {}
        }
    }

    pub fn toggle_expand(&mut self) {
        match self.selected_tree_item() {
            Some(TreeItem::Environment { name, .. }) => {
                let env_name = name.clone();
                if self.expanded_envs.contains(&env_name) {
                    self.expanded_envs.remove(&env_name);
                } else {
                    self.expanded_envs.insert(env_name);
                }
                self.rebuild_items();
            }
            Some(TreeItem::Host { name, env_name, .. }) => {
                let host_key = format!("{env_name}:{name}");
                if self.expanded_hosts.contains(&host_key) {
                    self.expanded_hosts.remove(&host_key);
                } else {
                    self.expanded_hosts.insert(host_key);
                }
                self.rebuild_items();
            }
            _ => {}
        }
    }

    pub fn cycle_focus(&mut self) {
        self.focus = self.focus.next();
    }

    pub fn cycle_focus_prev(&mut self) {
        self.focus = self.focus.prev();
    }

    pub fn set_focus(&mut self, panel: FocusPanel) {
        self.focus = panel;
    }

    pub fn add_log(&mut self, line: impl Into<String>) {
        self.command_log.push(line.into());
        self.log_scroll = 0;
    }

    pub fn scroll_log_up(&mut self, amount: usize) {
        let total = self.command_log.len();
        if total > 0 {
            self.log_scroll = (self.log_scroll + amount).min(total.saturating_sub(1));
        }
    }

    pub fn scroll_log_down(&mut self, amount: usize) {
        self.log_scroll = self.log_scroll.saturating_sub(amount);
    }

    pub fn scroll_log_to_top(&mut self) {
        let total = self.command_log.len();
        if total > 0 {
            self.log_scroll = total.saturating_sub(1);
        }
    }

    pub fn scroll_log_to_bottom(&mut self) {
        self.log_scroll = 0;
    }

    pub fn toggle_maximize_logs(&mut self) {
        self.logs_maximized = !self.logs_maximized;
        if self.logs_maximized {
            self.focus = FocusPanel::Log;
        }
    }

    pub fn toggle_help(&mut self) {
        if matches!(self.active_popup, ActivePopup::HelpModal(_)) {
            self.active_popup = ActivePopup::None;
        } else {
            self.active_popup = ActivePopup::HelpModal(HelpModalState {
                filter: String::new(),
                selected_index: 0,
                scroll: 0,
            });
        }
    }

    pub fn prompt_identity(&mut self, pending: PendingSecretAction) {
        let current_input = self
            .identity
            .as_ref()
            .map(|p| p.to_string_lossy().to_string())
            .unwrap_or_default();
        self.active_popup = ActivePopup::IdentityModal(IdentityModalState {
            input: current_input,
            pending,
            error: None,
        });
    }

    pub fn reload(&mut self, fleet: Fleet) {
        self.fleet = fleet;
        self.rebuild_items();
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn sample_fleet() -> Fleet {
        Fleet {
            environments: vec![Environment {
                name: "prod".to_string(),
                domain: "prod.rocky.internal".to_string(),
                hosts: vec![
                    Host {
                        name: "app".to_string(),
                        environment: "prod".to_string(),
                        services: vec!["caddy".to_string()],
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
                ],
                secrets: vec![Secret {
                    name: "jwt.age".to_string(),
                    environment: "prod".to_string(),
                    recipients: "2 recipients".to_string(),
                    services: vec!["postgres".to_string()],
                }],
                mesh_connected: true,
            }],
            services: vec![
                Service {
                    name: "caddy".to_string(),
                    kind: "systemd".to_string(),
                    target: "caddy.service".to_string(),
                    replicas: 1,
                    base_port: None,
                    environments: vec!["prod".to_string()],
                    hosts: vec!["prod:app".to_string()],
                    secrets: vec![],
                },
                Service {
                    name: "postgres".to_string(),
                    kind: "systemd".to_string(),
                    target: "postgresql.service".to_string(),
                    replicas: 1,
                    base_port: None,
                    environments: vec!["prod".to_string()],
                    hosts: vec!["prod:db".to_string()],
                    secrets: vec!["jwt.age".to_string()],
                },
            ],
        }
    }

    #[test]
    fn test_focus_panel_cycling() {
        let mut state = TuiState::new(sample_fleet(), None);
        assert_eq!(state.focus, FocusPanel::Environments);

        state.cycle_focus();
        assert_eq!(state.focus, FocusPanel::Services);

        state.cycle_focus();
        assert_eq!(state.focus, FocusPanel::Secrets);

        state.cycle_focus();
        assert_eq!(state.focus, FocusPanel::Log);

        state.cycle_focus();
        assert_eq!(state.focus, FocusPanel::Details);

        state.cycle_focus();
        assert_eq!(state.focus, FocusPanel::Status);

        state.cycle_focus();
        assert_eq!(state.focus, FocusPanel::Environments);

        state.set_focus(FocusPanel::Details);
        assert_eq!(state.focus, FocusPanel::Details);
    }

    #[test]
    fn test_log_scrolling() {
        let mut state = TuiState::new(sample_fleet(), None);
        for i in 0..20 {
            state.add_log(format!("line {i}"));
        }
        assert_eq!(state.log_scroll, 0);

        state.scroll_log_up(5);
        assert_eq!(state.log_scroll, 5);

        state.scroll_log_down(2);
        assert_eq!(state.log_scroll, 3);

        state.scroll_log_to_top();
        assert_eq!(state.log_scroll, 19);

        state.scroll_log_to_bottom();
        assert_eq!(state.log_scroll, 0);

        state.scroll_log_up(5);
        state.add_log("new output");
        assert_eq!(state.log_scroll, 0);
    }

    #[test]
    fn test_per_panel_navigation() {
        let mut state = TuiState::new(sample_fleet(), None);

        assert_eq!(state.env_host_index, 0);
        state.move_down();
        assert_eq!(state.env_host_index, 1);
        assert_eq!(state.selected_host().map(|h| h.name.as_str()), Some("app"));

        state.toggle_expand();
        state.move_down();
        assert_eq!(state.env_host_index, 2);
        assert_eq!(
            state.selected_service().map(|s| s.name.as_str()),
            Some("caddy")
        );
        assert_eq!(state.selected_host().map(|h| h.name.as_str()), Some("app"));

        state.move_up();
        state.toggle_expand();
        assert_eq!(state.tree_items().len(), 3);

        state.set_focus(FocusPanel::Services);
        assert_eq!(state.service_index, 0);
        state.move_down();
        assert_eq!(state.service_index, 1);
        assert_eq!(
            state.selected_service().map(|s| s.name.as_str()),
            Some("postgres")
        );

        state.set_focus(FocusPanel::Secrets);
        assert_eq!(state.secret_index, 0);
        assert_eq!(
            state.selected_secret().map(|s| s.name.as_str()),
            Some("jwt.age")
        );
    }
}
