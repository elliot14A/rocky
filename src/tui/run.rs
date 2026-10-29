use std::io::{stdout, Stdout};
use std::path::{Path, PathBuf};
use std::time::Duration;

use crossterm::event::{self, Event, KeyCode, KeyEvent, KeyModifiers};
use crossterm::execute;
use crossterm::terminal::{
    disable_raw_mode, enable_raw_mode, EnterAlternateScreen, LeaveAlternateScreen,
};
use ratatui::backend::CrosstermBackend;
use ratatui::Terminal;
use snafu::ResultExt;
use tap::prelude::*;

use super::action;
use super::draw::draw;
use super::load::load_fleet;
use super::state::{
    ActivePopup, EnvForm, FocusPanel, HostForm, SecretForm, ServiceForm, TreeItem, TuiState,
};
use crate::error::{Result, TerminalSnafu};
use crate::templates::service::ServiceKind;

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum KeyAction {
    Quit,
    MoveDown,
    MoveUp,
    CycleFocus,
    CycleFocusPrev,
    SetFocus(FocusPanel),
    ToggleExpand,
    ClosePopup,
    NextFormField,
    PrevFormField,
    ToggleFormField,
    InputChar(char),
    InputBackspace,
    SubmitForm,
    ConfirmJoinMesh,
    NewEntity,
    SelectNewEntity(NewEntityType),
    Reload,
    JoinMesh,
    Edit,
    Ssh,
    Logs,
    Check,
    Deploy,
    ScrollLogUp(usize),
    ScrollLogDown(usize),
    ScrollLogTop,
    ScrollLogBottom,
    ClearLog,
    ToggleMaximizeLogs,
    ToggleHelp,
    ClearHelpFilter,
    HelpFilterChar(char),
    HelpFilterBackspace,
    HelpNext,
    HelpPrev,
    HelpPageUp,
    HelpPageDown,
    ViewSecret,
    ScrollSecretViewerUp(usize),
    ScrollSecretViewerDown(usize),
    PromptIdentity(crate::tui::state::PendingSecretAction),
    IdentityInputChar(char),
    IdentityInputBackspace,
    SubmitIdentity,
    Ignore,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum NewEntityType {
    Environment,
    Host,
    Service,
    Secret,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ActionEffect {
    Continue,
    Quit,
    Reload,
    ExecuteJoinMesh {
        env_name: String,
    },
    ExecuteEdit,
    ExecuteViewSecret,
    ExecuteCreateHost(HostForm),
    ExecuteCreateService(ServiceForm),
    ExecuteCreateSecret(SecretForm),
    ExecuteCreateEnv(EnvForm),
    ExecuteSsh {
        env_name: String,
        host_name: String,
    },
    ExecuteLogs {
        env_name: String,
        host_name: String,
        service_unit: Option<String>,
    },
    ExecuteCheck,
    ExecuteDeploy {
        env_name: String,
        target_host: Option<String>,
    },
}

pub fn classify_key_event(event: KeyEvent, state: &TuiState) -> KeyAction {
    if event.kind != crossterm::event::KeyEventKind::Press {
        return KeyAction::Ignore;
    }

    if event.modifiers.contains(KeyModifiers::CONTROL) && event.code == KeyCode::Char('c') {
        return KeyAction::Quit;
    }

    match &state.active_popup {
        ActivePopup::HelpModal(modal) => match event.code {
            KeyCode::Esc => {
                if modal.filter.is_empty() {
                    KeyAction::ClosePopup
                } else {
                    KeyAction::ClearHelpFilter
                }
            }
            KeyCode::Char('?') => KeyAction::ClosePopup,
            KeyCode::Down => KeyAction::HelpNext,
            KeyCode::Up => KeyAction::HelpPrev,
            KeyCode::PageDown => KeyAction::HelpPageDown,
            KeyCode::PageUp => KeyAction::HelpPageUp,
            KeyCode::Backspace => KeyAction::HelpFilterBackspace,
            KeyCode::Enter => KeyAction::ClosePopup,
            KeyCode::Char(c) => KeyAction::HelpFilterChar(c),
            _ => KeyAction::Ignore,
        },
        ActivePopup::SecretViewer(_) => match event.code {
            KeyCode::Esc | KeyCode::Char('q') | KeyCode::Enter => KeyAction::ClosePopup,
            KeyCode::Char('e') => KeyAction::Edit,
            KeyCode::Char('j') | KeyCode::Down => KeyAction::ScrollSecretViewerDown(1),
            KeyCode::Char('k') | KeyCode::Up => KeyAction::ScrollSecretViewerUp(1),
            KeyCode::PageDown => KeyAction::ScrollSecretViewerDown(10),
            KeyCode::PageUp => KeyAction::ScrollSecretViewerUp(10),
            _ => KeyAction::Ignore,
        },
        ActivePopup::IdentityModal(_) => match event.code {
            KeyCode::Esc => KeyAction::ClosePopup,
            KeyCode::Enter => KeyAction::SubmitIdentity,
            KeyCode::Backspace => KeyAction::IdentityInputBackspace,
            KeyCode::Char(c) => KeyAction::IdentityInputChar(c),
            _ => KeyAction::Ignore,
        },
        ActivePopup::NewEntityMenu => match event.code {
            KeyCode::Esc | KeyCode::Char('q') => KeyAction::ClosePopup,
            KeyCode::Char('1') | KeyCode::Char('e') => {
                KeyAction::SelectNewEntity(NewEntityType::Environment)
            }
            KeyCode::Char('2') | KeyCode::Char('h') => {
                KeyAction::SelectNewEntity(NewEntityType::Host)
            }
            KeyCode::Char('3') | KeyCode::Char('s') => {
                KeyAction::SelectNewEntity(NewEntityType::Service)
            }
            KeyCode::Char('4') | KeyCode::Char('x') => {
                KeyAction::SelectNewEntity(NewEntityType::Secret)
            }
            _ => KeyAction::Ignore,
        },
        ActivePopup::HostModal(_)
        | ActivePopup::ServiceModal(_)
        | ActivePopup::SecretModal(_)
        | ActivePopup::EnvModal(_) => match event.code {
            KeyCode::Esc => KeyAction::ClosePopup,
            KeyCode::Enter => KeyAction::SubmitForm,
            KeyCode::Tab | KeyCode::Down => KeyAction::NextFormField,
            KeyCode::BackTab | KeyCode::Up => KeyAction::PrevFormField,
            KeyCode::Char(' ') => KeyAction::ToggleFormField,
            KeyCode::Backspace => KeyAction::InputBackspace,
            KeyCode::Char(c) => KeyAction::InputChar(c),
            _ => KeyAction::Ignore,
        },
        ActivePopup::ConfirmJoinMesh { .. } => match event.code {
            KeyCode::Esc | KeyCode::Char('q') | KeyCode::Char('n') => KeyAction::ClosePopup,
            KeyCode::Enter | KeyCode::Char('j') | KeyCode::Char('y') => KeyAction::ConfirmJoinMesh,
            _ => KeyAction::Ignore,
        },
        ActivePopup::Error(_) => match event.code {
            KeyCode::Esc | KeyCode::Enter | KeyCode::Char(' ') | KeyCode::Char('q') => {
                KeyAction::ClosePopup
            }
            _ => KeyAction::Ignore,
        },
        ActivePopup::None => {
            if state.logs_maximized {
                return match event.code {
                    KeyCode::Esc | KeyCode::Char('m') => KeyAction::ToggleMaximizeLogs,
                    KeyCode::Char('j') | KeyCode::Down => KeyAction::ScrollLogDown(1),
                    KeyCode::Char('k') | KeyCode::Up => KeyAction::ScrollLogUp(1),
                    KeyCode::PageDown => KeyAction::ScrollLogDown(10),
                    KeyCode::PageUp => KeyAction::ScrollLogUp(10),
                    KeyCode::Char('g') => KeyAction::ScrollLogTop,
                    KeyCode::Char('G') => KeyAction::ScrollLogBottom,
                    KeyCode::Char('c') => KeyAction::ClearLog,
                    KeyCode::Char('?') | KeyCode::Char('h') => KeyAction::ToggleHelp,
                    KeyCode::Char('q') => KeyAction::Quit,
                    _ => KeyAction::Ignore,
                };
            }

            match event.code {
                KeyCode::Char('?') | KeyCode::Char('h') => KeyAction::ToggleHelp,
                KeyCode::Char('m') => KeyAction::ToggleMaximizeLogs,
                KeyCode::Char('q') => KeyAction::Quit,
                KeyCode::Esc => KeyAction::Quit,
                KeyCode::Tab => KeyAction::CycleFocus,
                KeyCode::BackTab => KeyAction::CycleFocusPrev,
                KeyCode::Char(d) if FocusPanel::from_digit(d).is_some() => {
                    if let Some(panel) = FocusPanel::from_digit(d) {
                        KeyAction::SetFocus(panel)
                    } else {
                        KeyAction::Ignore
                    }
                }
                KeyCode::Char('r') => KeyAction::Reload,

                KeyCode::PageUp => KeyAction::ScrollLogUp(10),
                KeyCode::PageDown => KeyAction::ScrollLogDown(10),

                _ => match state.focus {
                    FocusPanel::Secrets => match event.code {
                        KeyCode::Char('j') | KeyCode::Down => KeyAction::MoveDown,
                        KeyCode::Char('k') | KeyCode::Up => KeyAction::MoveUp,
                        KeyCode::Enter | KeyCode::Char('v') => {
                            if state.identity.is_none() {
                                KeyAction::PromptIdentity(
                                    crate::tui::state::PendingSecretAction::View,
                                )
                            } else {
                                KeyAction::ViewSecret
                            }
                        }
                        KeyCode::Char('e') => {
                            if state.identity.is_none() {
                                KeyAction::PromptIdentity(
                                    crate::tui::state::PendingSecretAction::Edit,
                                )
                            } else {
                                KeyAction::Edit
                            }
                        }
                        KeyCode::Char('i') => {
                            KeyAction::PromptIdentity(crate::tui::state::PendingSecretAction::None)
                        }
                        KeyCode::Char('n') => KeyAction::NewEntity,
                        _ => KeyAction::Ignore,
                    },
                    FocusPanel::Services => match event.code {
                        KeyCode::Char('j') | KeyCode::Down => KeyAction::MoveDown,
                        KeyCode::Char('k') | KeyCode::Up => KeyAction::MoveUp,
                        KeyCode::Char('l') => KeyAction::Logs,
                        KeyCode::Char('e') => KeyAction::Edit,
                        KeyCode::Char('n') => KeyAction::NewEntity,
                        _ => KeyAction::Ignore,
                    },
                    FocusPanel::Environments => match event.code {
                        KeyCode::Char('j') | KeyCode::Down => KeyAction::MoveDown,
                        KeyCode::Char('k') | KeyCode::Up => KeyAction::MoveUp,
                        KeyCode::Char(' ') | KeyCode::Enter => KeyAction::ToggleExpand,
                        KeyCode::Char('s') => KeyAction::Ssh,
                        KeyCode::Char('l') => KeyAction::Logs,
                        KeyCode::Char('e') => KeyAction::Edit,
                        KeyCode::Char('J') => KeyAction::JoinMesh,
                        KeyCode::Char('u') => KeyAction::Deploy,
                        KeyCode::Char('c') => KeyAction::Check,
                        KeyCode::Char('n') => KeyAction::NewEntity,
                        _ => KeyAction::Ignore,
                    },
                    FocusPanel::Log => match event.code {
                        KeyCode::Char('j') | KeyCode::Down => KeyAction::ScrollLogDown(1),
                        KeyCode::Char('k') | KeyCode::Up => KeyAction::ScrollLogUp(1),
                        KeyCode::PageDown => KeyAction::ScrollLogDown(10),
                        KeyCode::PageUp => KeyAction::ScrollLogUp(10),
                        KeyCode::Char('g') => KeyAction::ScrollLogTop,
                        KeyCode::Char('G') => KeyAction::ScrollLogBottom,
                        KeyCode::Enter => KeyAction::ToggleMaximizeLogs,
                        KeyCode::Char('c') => KeyAction::ClearLog,
                        _ => KeyAction::Ignore,
                    },
                    FocusPanel::Details => match event.code {
                        KeyCode::Char('j') | KeyCode::Down => KeyAction::MoveDown,
                        KeyCode::Char('k') | KeyCode::Up => KeyAction::MoveUp,
                        KeyCode::Char('e') => KeyAction::Edit,
                        _ => KeyAction::Ignore,
                    },
                    FocusPanel::Status => match event.code {
                        KeyCode::Char('c') => KeyAction::Check,
                        KeyCode::Char('u') => KeyAction::Deploy,
                        _ => KeyAction::Ignore,
                    },
                },
            }
        }
    }
}

pub fn apply_action(state: &mut TuiState, action: KeyAction) -> ActionEffect {
    match action {
        KeyAction::Quit => ActionEffect::Quit,
        KeyAction::Reload => ActionEffect::Reload,
        KeyAction::CycleFocus => {
            state.cycle_focus();
            ActionEffect::Continue
        }
        KeyAction::CycleFocusPrev => {
            state.cycle_focus_prev();
            ActionEffect::Continue
        }
        KeyAction::SetFocus(panel) => {
            state.set_focus(panel);
            ActionEffect::Continue
        }
        KeyAction::MoveDown => {
            state.move_down();
            ActionEffect::Continue
        }
        KeyAction::MoveUp => {
            state.move_up();
            ActionEffect::Continue
        }
        KeyAction::ToggleExpand => {
            if state.focus == FocusPanel::Environments {
                state.toggle_expand();
            }
            ActionEffect::Continue
        }
        KeyAction::ClosePopup => {
            state.active_popup = ActivePopup::None;
            ActionEffect::Continue
        }
        KeyAction::NewEntity => {
            state.active_popup = ActivePopup::NewEntityMenu;
            ActionEffect::Continue
        }
        KeyAction::SelectNewEntity(kind) => {
            let env_name = state
                .selected_env()
                .map(|e| e.name.clone())
                .or_else(|| state.fleet.environments.first().map(|e| e.name.clone()))
                .unwrap_or_default();

            let selected_host_name = state.selected_host().map(|h| h.name.clone());

            match kind {
                NewEntityType::Environment => {
                    state.active_popup = ActivePopup::EnvModal(EnvForm {
                        name: String::new(),
                    });
                }
                NewEntityType::Host => {
                    state.active_popup = ActivePopup::HostModal(HostForm {
                        name: String::new(),
                        environment: env_name,
                        provider: "hetzner".to_string(),
                        active_field: 0,
                    });
                }
                NewEntityType::Service => {
                    state.active_popup = ActivePopup::ServiceModal(ServiceForm {
                        name: String::new(),
                        kind: ServiceKind::Systemd,
                        host: selected_host_name,
                        port: String::new(),
                        replicas: 1,
                        active_field: 0,
                    });
                }
                NewEntityType::Secret => {
                    state.active_popup = ActivePopup::SecretModal(SecretForm {
                        name: String::new(),
                        environment: env_name,
                        all_hosts: true,
                        active_field: 0,
                    });
                }
            }
            ActionEffect::Continue
        }
        KeyAction::NextFormField => {
            match &mut state.active_popup {
                ActivePopup::HostModal(f) => f.active_field = (f.active_field + 1) % 3,
                ActivePopup::ServiceModal(f) => f.active_field = (f.active_field + 1) % 5,
                ActivePopup::SecretModal(f) => f.active_field = (f.active_field + 1) % 3,
                _ => {}
            }
            ActionEffect::Continue
        }
        KeyAction::PrevFormField => {
            match &mut state.active_popup {
                ActivePopup::HostModal(f) => {
                    f.active_field = if f.active_field == 0 {
                        2
                    } else {
                        f.active_field - 1
                    }
                }
                ActivePopup::ServiceModal(f) => {
                    f.active_field = if f.active_field == 0 {
                        4
                    } else {
                        f.active_field - 1
                    }
                }
                ActivePopup::SecretModal(f) => {
                    f.active_field = if f.active_field == 0 {
                        2
                    } else {
                        f.active_field - 1
                    }
                }
                _ => {}
            }
            ActionEffect::Continue
        }
        KeyAction::ToggleFormField => {
            let env_names: Vec<String> = state
                .fleet
                .environments
                .iter()
                .map(|e| e.name.clone())
                .collect();
            let hosts: Vec<String> = state
                .selected_env()
                .map(|e| e.hosts.iter().map(|h| h.name.clone()).collect())
                .unwrap_or_default();

            match &mut state.active_popup {
                ActivePopup::HostModal(f) => {
                    if f.active_field == 1 && !env_names.is_empty() {
                        let cur_pos = env_names
                            .iter()
                            .position(|e| e == &f.environment)
                            .unwrap_or(0);
                        f.environment = env_names[(cur_pos + 1) % env_names.len()].clone();
                    } else if f.active_field == 2 {
                        f.provider = if f.provider == "hetzner" {
                            "default".to_string()
                        } else {
                            "hetzner".to_string()
                        };
                    } else {
                        f.name.push(' ');
                    }
                }
                ActivePopup::ServiceModal(f) => {
                    if f.active_field == 1 {
                        f.kind = match f.kind {
                            ServiceKind::Systemd => ServiceKind::Podman,
                            ServiceKind::Podman => ServiceKind::Systemd,
                        };
                    } else if f.active_field == 2 && !hosts.is_empty() {
                        let cur_host = f.host.as_deref().unwrap_or("");
                        let cur_pos = hosts
                            .iter()
                            .position(|h| h == cur_host)
                            .map(|p| p + 1)
                            .unwrap_or(0);
                        f.host = if cur_pos >= hosts.len() {
                            None
                        } else {
                            Some(hosts[cur_pos].clone())
                        };
                    } else if f.active_field == 4 {
                        f.replicas = if f.replicas >= 10 { 1 } else { f.replicas + 1 };
                    } else if f.active_field == 0 {
                        f.name.push(' ');
                    }
                }
                ActivePopup::SecretModal(f) => {
                    if f.active_field == 1 && !env_names.is_empty() {
                        let cur_pos = env_names
                            .iter()
                            .position(|e| e == &f.environment)
                            .unwrap_or(0);
                        f.environment = env_names[(cur_pos + 1) % env_names.len()].clone();
                    } else if f.active_field == 2 {
                        f.all_hosts = !f.all_hosts;
                    } else {
                        f.name.push(' ');
                    }
                }
                ActivePopup::EnvModal(f) => {
                    f.name.push(' ');
                }
                _ => {}
            }
            ActionEffect::Continue
        }
        KeyAction::InputChar(c) => {
            match &mut state.active_popup {
                ActivePopup::HostModal(f) if f.active_field == 0 => f.name.push(c),
                ActivePopup::ServiceModal(f) => {
                    if f.active_field == 0 {
                        f.name.push(c);
                    } else if f.active_field == 3 && c.is_ascii_digit() {
                        f.port.push(c);
                    } else if f.active_field == 4 && c.is_ascii_digit() {
                        let digit = (c as u32) - ('0' as u32);
                        f.replicas = if f.replicas == 1 && digit > 0 {
                            digit
                        } else {
                            f.replicas
                                .saturating_mul(10)
                                .saturating_add(digit)
                                .clamp(1, 999)
                        };
                    }
                }
                ActivePopup::SecretModal(f) if f.active_field == 0 => f.name.push(c),
                ActivePopup::EnvModal(f) => f.name.push(c),
                _ => {}
            }
            ActionEffect::Continue
        }
        KeyAction::InputBackspace => {
            match &mut state.active_popup {
                ActivePopup::HostModal(f) if f.active_field == 0 => {
                    f.name.pop();
                }
                ActivePopup::ServiceModal(f) => {
                    if f.active_field == 0 {
                        f.name.pop();
                    } else if f.active_field == 3 {
                        f.port.pop();
                    } else if f.active_field == 4 {
                        f.replicas = (f.replicas / 10).max(1);
                    }
                }
                ActivePopup::SecretModal(f) if f.active_field == 0 => {
                    f.name.pop();
                }
                ActivePopup::EnvModal(f) => {
                    f.name.pop();
                }
                _ => {}
            }
            ActionEffect::Continue
        }
        KeyAction::SubmitForm => {
            let popup = std::mem::take(&mut state.active_popup);
            match popup {
                ActivePopup::HostModal(form) => {
                    if !form.name.trim().is_empty() {
                        ActionEffect::ExecuteCreateHost(form)
                    } else {
                        ActionEffect::Continue
                    }
                }
                ActivePopup::ServiceModal(form) => {
                    if !form.name.trim().is_empty() {
                        ActionEffect::ExecuteCreateService(form)
                    } else {
                        ActionEffect::Continue
                    }
                }
                ActivePopup::SecretModal(form) => {
                    if !form.name.trim().is_empty() {
                        ActionEffect::ExecuteCreateSecret(form)
                    } else {
                        ActionEffect::Continue
                    }
                }
                ActivePopup::EnvModal(form) => {
                    if !form.name.trim().is_empty() {
                        ActionEffect::ExecuteCreateEnv(form)
                    } else {
                        ActionEffect::Continue
                    }
                }
                _ => ActionEffect::Continue,
            }
        }
        KeyAction::JoinMesh => {
            if let Some(env) = state.selected_env() {
                if env.mesh_connected {
                    state.add_log(format!(
                        "> workstation already joined to mesh '{}'",
                        env.name
                    ));
                } else {
                    state.active_popup = ActivePopup::ConfirmJoinMesh {
                        env_name: env.name.clone(),
                    };
                }
            }
            ActionEffect::Continue
        }
        KeyAction::ConfirmJoinMesh => {
            let env_name = match &state.active_popup {
                ActivePopup::ConfirmJoinMesh { env_name } => Some(env_name.clone()),
                _ => None,
            };
            state.active_popup = ActivePopup::None;
            if let Some(name) = env_name {
                ActionEffect::ExecuteJoinMesh { env_name: name }
            } else {
                ActionEffect::Continue
            }
        }
        KeyAction::Ssh => {
            if let Some(host) = state.selected_host() {
                let env_name = host.environment.clone();
                let host_name = host.name.clone();
                let is_connected = state
                    .selected_env()
                    .map(|e| e.mesh_connected)
                    .unwrap_or(false);

                if !is_connected {
                    state.active_popup = ActivePopup::ConfirmJoinMesh { env_name };
                    ActionEffect::Continue
                } else {
                    ActionEffect::ExecuteSsh {
                        env_name,
                        host_name,
                    }
                }
            } else {
                state.add_log("> select a host in panel [2] to connect via SSH");
                ActionEffect::Continue
            }
        }
        KeyAction::Logs => {
            let (target_env, target_host, service_unit) = match state.focus {
                FocusPanel::Services => {
                    if let Some(svc) = state.selected_service() {
                        let host_str = state
                            .selected_host()
                            .and_then(|h| {
                                svc.hosts
                                    .iter()
                                    .find(|entry| entry.ends_with(&format!(":{}", h.name)))
                            })
                            .or_else(|| svc.hosts.first());

                        if let Some(host_str) = host_str {
                            let parts: Vec<&str> = host_str.split(':').collect();
                            let (env_name, host_name) = if parts.len() == 2 {
                                (parts[0].to_string(), parts[1].to_string())
                            } else {
                                (
                                    svc.environments
                                        .first()
                                        .cloned()
                                        .or_else(|| {
                                            state.fleet.environments.first().map(|e| e.name.clone())
                                        })
                                        .unwrap_or_default(),
                                    parts[0].to_string(),
                                )
                            };
                            (Some(env_name), Some(host_name), Some(svc.name.clone()))
                        } else {
                            (None, None, None)
                        }
                    } else {
                        (None, None, None)
                    }
                }
                _ => {
                    if let Some(host) = state.selected_host() {
                        let env_name = host.environment.clone();
                        let host_name = host.name.clone();
                        let unit = match state.selected_tree_item() {
                            Some(TreeItem::HostService { name, .. }) => Some(name.clone()),
                            _ => None,
                        };
                        (Some(env_name), Some(host_name), unit)
                    } else {
                        (None, None, None)
                    }
                }
            };

            if let (Some(env_name), Some(host_name)) = (target_env, target_host) {
                let is_connected = state
                    .fleet
                    .environments
                    .iter()
                    .find(|e| e.name == env_name)
                    .map(|e| e.mesh_connected)
                    .unwrap_or(false);

                if !is_connected {
                    state.active_popup = ActivePopup::ConfirmJoinMesh { env_name };
                    ActionEffect::Continue
                } else {
                    ActionEffect::ExecuteLogs {
                        env_name,
                        host_name,
                        service_unit,
                    }
                }
            } else {
                state.add_log("> select a host or service in panel [2] or [3] to stream logs");
                ActionEffect::Continue
            }
        }
        KeyAction::Edit => ActionEffect::ExecuteEdit,
        KeyAction::Check => ActionEffect::ExecuteCheck,
        KeyAction::Deploy => {
            if let Some(env) = state.selected_env() {
                if !env.mesh_connected {
                    state.active_popup = ActivePopup::ConfirmJoinMesh {
                        env_name: env.name.clone(),
                    };
                    ActionEffect::Continue
                } else {
                    let target_host = state.selected_host().map(|h| h.name.clone());
                    ActionEffect::ExecuteDeploy {
                        env_name: env.name.clone(),
                        target_host,
                    }
                }
            } else {
                state.add_log("> select an environment in panel [2] to deploy");
                ActionEffect::Continue
            }
        }
        KeyAction::ScrollLogUp(amount) => {
            state.scroll_log_up(amount);
            ActionEffect::Continue
        }
        KeyAction::ScrollLogDown(amount) => {
            state.scroll_log_down(amount);
            ActionEffect::Continue
        }
        KeyAction::ScrollLogTop => {
            state.scroll_log_to_top();
            ActionEffect::Continue
        }
        KeyAction::ScrollLogBottom => {
            state.scroll_log_to_bottom();
            ActionEffect::Continue
        }
        KeyAction::ClearLog => {
            state.command_log.clear();
            state.log_scroll = 0;
            ActionEffect::Continue
        }
        KeyAction::ToggleMaximizeLogs => {
            state.toggle_maximize_logs();
            ActionEffect::Continue
        }
        KeyAction::ToggleHelp => {
            state.toggle_help();
            ActionEffect::Continue
        }
        KeyAction::ClearHelpFilter => {
            if let ActivePopup::HelpModal(m) = &mut state.active_popup {
                m.filter.clear();
                m.selected_index = 0;
            }
            ActionEffect::Continue
        }
        KeyAction::HelpFilterChar(c) => {
            if let ActivePopup::HelpModal(m) = &mut state.active_popup {
                m.filter.push(c);
                m.selected_index = 0;
            }
            ActionEffect::Continue
        }
        KeyAction::HelpFilterBackspace => {
            if let ActivePopup::HelpModal(m) = &mut state.active_popup {
                m.filter.pop();
                m.selected_index = 0;
            }
            ActionEffect::Continue
        }
        KeyAction::HelpNext => {
            if let ActivePopup::HelpModal(m) = &mut state.active_popup {
                let all_items = crate::tui::draw::help::get_keybindings(state.focus);
                let filtered = crate::tui::draw::help::filter_keybindings(&all_items, &m.filter);
                if !filtered.is_empty() {
                    m.selected_index = (m.selected_index + 1) % filtered.len();
                }
            }
            ActionEffect::Continue
        }
        KeyAction::HelpPrev => {
            if let ActivePopup::HelpModal(m) = &mut state.active_popup {
                let all_items = crate::tui::draw::help::get_keybindings(state.focus);
                let filtered = crate::tui::draw::help::filter_keybindings(&all_items, &m.filter);
                if !filtered.is_empty() {
                    if m.selected_index == 0 {
                        m.selected_index = filtered.len() - 1;
                    } else {
                        m.selected_index -= 1;
                    }
                }
            }
            ActionEffect::Continue
        }
        KeyAction::HelpPageDown => {
            if let ActivePopup::HelpModal(m) = &mut state.active_popup {
                let all_items = crate::tui::draw::help::get_keybindings(state.focus);
                let filtered = crate::tui::draw::help::filter_keybindings(&all_items, &m.filter);
                if !filtered.is_empty() {
                    m.selected_index = (m.selected_index + 10).min(filtered.len() - 1);
                }
            }
            ActionEffect::Continue
        }
        KeyAction::HelpPageUp => {
            if let ActivePopup::HelpModal(m) = &mut state.active_popup {
                m.selected_index = m.selected_index.saturating_sub(10);
            }
            ActionEffect::Continue
        }
        KeyAction::ViewSecret => ActionEffect::ExecuteViewSecret,
        KeyAction::ScrollSecretViewerUp(amount) => {
            if let ActivePopup::SecretViewer(v) = &mut state.active_popup {
                v.scroll = v.scroll.saturating_sub(amount);
            }
            ActionEffect::Continue
        }
        KeyAction::ScrollSecretViewerDown(amount) => {
            if let ActivePopup::SecretViewer(v) = &mut state.active_popup {
                let total_lines = v.content.lines().count();
                let max_scroll = total_lines.saturating_sub(1);
                v.scroll = v.scroll.saturating_add(amount).min(max_scroll);
            }
            ActionEffect::Continue
        }
        KeyAction::PromptIdentity(pending) => {
            state.prompt_identity(pending);
            ActionEffect::Continue
        }
        KeyAction::IdentityInputChar(c) => {
            if let ActivePopup::IdentityModal(m) = &mut state.active_popup {
                m.input.push(c);
                m.error = None;
            }
            ActionEffect::Continue
        }
        KeyAction::IdentityInputBackspace => {
            if let ActivePopup::IdentityModal(m) = &mut state.active_popup {
                m.input.pop();
                m.error = None;
            }
            ActionEffect::Continue
        }
        KeyAction::SubmitIdentity => {
            let (input, pending) = if let ActivePopup::IdentityModal(m) = &state.active_popup {
                (m.input.trim().to_string(), m.pending.clone())
            } else {
                return ActionEffect::Continue;
            };

            if input.is_empty() {
                if let ActivePopup::IdentityModal(m) = &mut state.active_popup {
                    m.error = Some("Identity key path cannot be empty".to_string());
                }
                return ActionEffect::Continue;
            }

            let expanded = if let Some(rest) = input.strip_prefix("~/") {
                if let Ok(home) = std::env::var("HOME") {
                    PathBuf::from(home).join(rest)
                } else {
                    PathBuf::from(&input)
                }
            } else {
                PathBuf::from(&input)
            };

            if !expanded.is_file() {
                if let ActivePopup::IdentityModal(m) = &mut state.active_popup {
                    m.error = Some(format!("File '{}' not found", expanded.display()));
                }
                return ActionEffect::Continue;
            }

            state.identity = Some(expanded.clone());
            state.active_popup = ActivePopup::None;
            state.add_log(format!("> identity set to '{}'", expanded.display()));

            match pending {
                crate::tui::state::PendingSecretAction::View => ActionEffect::ExecuteViewSecret,
                crate::tui::state::PendingSecretAction::Edit => ActionEffect::ExecuteEdit,
                crate::tui::state::PendingSecretAction::None => ActionEffect::Continue,
            }
        }
        KeyAction::Ignore => ActionEffect::Continue,
    }
}

pub struct SuspendGuard<'a> {
    terminal: &'a mut Terminal<CrosstermBackend<Stdout>>,
}

impl<'a> SuspendGuard<'a> {
    pub fn new(terminal: &'a mut Terminal<CrosstermBackend<Stdout>>) -> Result<Self> {
        disable_raw_mode().context(TerminalSnafu)?;
        execute!(terminal.backend_mut(), LeaveAlternateScreen).context(TerminalSnafu)?;
        let _ = terminal.show_cursor();
        Ok(Self { terminal })
    }
}

impl<'a> Drop for SuspendGuard<'a> {
    fn drop(&mut self) {
        let _ = enable_raw_mode();
        let _ = execute!(self.terminal.backend_mut(), EnterAlternateScreen);
        let _ = self.terminal.hide_cursor();
        let _ = self.terminal.clear();
    }
}

pub struct TerminalSession {
    terminal: Terminal<CrosstermBackend<Stdout>>,
}

impl TerminalSession {
    pub fn new() -> Result<Self> {
        let original_hook = std::panic::take_hook();
        std::panic::set_hook(Box::new(move |panic_info| {
            let _ = disable_raw_mode();
            let _ = execute!(std::io::stdout(), LeaveAlternateScreen);
            let _ = execute!(std::io::stdout(), crossterm::cursor::Show);
            original_hook(panic_info);
        }));

        enable_raw_mode().context(TerminalSnafu)?;

        let mut stdout = stdout();
        execute!(stdout, EnterAlternateScreen).context(TerminalSnafu)?;

        let backend = CrosstermBackend::new(stdout);
        let mut terminal = Terminal::new(backend).context(TerminalSnafu)?;
        let _ = terminal.hide_cursor();

        Ok(Self { terminal })
    }

    pub async fn suspend<F, Fut, R>(&mut self, action: F) -> Result<R>
    where
        F: FnOnce() -> Fut,
        Fut: std::future::Future<Output = Result<R>>,
    {
        let guard = SuspendGuard::new(&mut self.terminal)?;
        let result = action().await;
        drop(guard);
        result
    }

    pub fn draw(&mut self, state: &TuiState) -> Result<()> {
        self.terminal
            .draw(|f| draw(f, state))
            .context(TerminalSnafu)?;
        Ok(())
    }
}

impl Drop for TerminalSession {
    fn drop(&mut self) {
        let _ = disable_raw_mode();
        let _ = execute!(self.terminal.backend_mut(), LeaveAlternateScreen);
        let _ = self.terminal.show_cursor();
    }
}

pub async fn run(fleet_root: &Path, identity: Option<&Path>) -> Result<()> {
    let fleet = load_fleet(fleet_root).await?;
    let mut state = TuiState::new(fleet, identity.map(Path::to_path_buf));
    let mut session = TerminalSession::new()?;

    loop {
        session.draw(&state)?;

        if event::poll(Duration::from_millis(100)).context(TerminalSnafu)? {
            if let Event::Key(key_event) = event::read().context(TerminalSnafu)? {
                let action = classify_key_event(key_event, &state);
                let effect = apply_action(&mut state, action);

                match effect {
                    ActionEffect::Quit => break,
                    ActionEffect::Reload => {
                        let _ = load_fleet(fleet_root)
                            .await
                            .tap_ok(|fresh| {
                                state.reload(fresh.clone());
                                state.add_log("> reloaded fleet items");
                            })
                            .tap_err(|e| {
                                state.add_log(format!("> reload failed: {e}"));
                            });
                    }
                    ActionEffect::ExecuteJoinMesh { env_name } => {
                        let id_ref = state.identity.clone();
                        state.add_log(format!("> rocky mesh join {env_name}"));
                        let res = session
                            .suspend(|| async {
                                println!("Joining fleet mesh for '{env_name}'...\n");
                                let r =
                                    action::join::execute(fleet_root, &env_name, id_ref.as_deref())
                                        .await;
                                println!("\nPress Enter to return to Rocky...");
                                let mut buf = String::new();
                                let _ = std::io::stdin().read_line(&mut buf);
                                r
                            })
                            .await;

                        match res {
                            Ok(()) => {
                                state.add_log(format!("joined mesh for '{env_name}'"));
                                if let Ok(fresh) = load_fleet(fleet_root).await {
                                    state.reload(fresh);
                                }
                            }
                            Err(e) => state.add_log(format!("join mesh failed: {e}")),
                        }
                    }
                    ActionEffect::ExecuteViewSecret => {
                        if state.identity.is_none() {
                            state.prompt_identity(crate::tui::state::PendingSecretAction::View);
                        } else {
                            match action::show::show_selected_secret(fleet_root, &state).await {
                                Ok(content) => {
                                    if let Some(sec) = state.selected_secret() {
                                        state.active_popup = ActivePopup::SecretViewer(
                                            crate::tui::state::SecretViewerState {
                                                name: sec.name.clone(),
                                                environment: sec.environment.clone(),
                                                content,
                                                scroll: 0,
                                            },
                                        );
                                    }
                                }
                                Err(e) => {
                                    state.add_log(format!("> failed to decrypt secret: {e}"));
                                    if e.to_string().contains("identity") {
                                        state.prompt_identity(
                                            crate::tui::state::PendingSecretAction::View,
                                        );
                                    } else {
                                        state.active_popup = ActivePopup::Error(format!(
                                            "Failed to decrypt secret:\n\n{e}"
                                        ));
                                    }
                                }
                            }
                        }
                    }
                    ActionEffect::ExecuteEdit => {
                        if state.focus == FocusPanel::Secrets && state.identity.is_none() {
                            state.prompt_identity(crate::tui::state::PendingSecretAction::Edit);
                            continue;
                        }

                        let res = session
                            .suspend(|| async { action::edit::execute(fleet_root, &state).await })
                            .await;

                        state.active_popup = ActivePopup::None;
                        match res {
                            Ok(true) => {
                                state.add_log("> saved changes");
                                if let Ok(fresh) = load_fleet(fleet_root).await {
                                    state.reload(fresh);
                                }
                            }
                            Ok(false) => state.add_log("> no changes made"),
                            Err(e) => state.add_log(format!("> edit failed: {e}")),
                        }
                    }
                    ActionEffect::ExecuteCreateHost(form) => {
                        state.add_log(format!("> creating host '{}'...", form.name));
                        match action::new::create_host_entity(
                            fleet_root,
                            &form.name,
                            &form.environment,
                            Some(&form.provider),
                        )
                        .await
                        {
                            Ok(msg) => {
                                state.add_log(msg);
                                if let Ok(fresh) = load_fleet(fleet_root).await {
                                    state.reload(fresh);
                                }
                            }
                            Err(e) => state.add_log(format!("failed to create host: {e}")),
                        }
                    }
                    ActionEffect::ExecuteCreateService(form) => {
                        let port_opt = form.port.parse::<u16>().ok();
                        state.add_log(format!("> creating service '{}'...", form.name));
                        match action::new::create_service_entity(
                            fleet_root,
                            &form.name,
                            form.kind,
                            form.host.as_deref(),
                            port_opt,
                            form.replicas,
                            state.selected_env().map(|e| e.name.as_str()),
                        )
                        .await
                        {
                            Ok(msg) => {
                                state.add_log(msg);
                                if let Ok(fresh) = load_fleet(fleet_root).await {
                                    state.reload(fresh);
                                }
                            }
                            Err(e) => state.add_log(format!("failed to create service: {e}")),
                        }
                    }
                    ActionEffect::ExecuteCreateSecret(form) => {
                        state.add_log(format!("> creating secret '{}'...", form.name));
                        let id_ref = state.identity.clone();
                        let res = session
                            .suspend(|| async {
                                action::new::create_secret_entity(
                                    fleet_root,
                                    &form.name,
                                    &form.environment,
                                    form.all_hosts,
                                    id_ref.as_deref(),
                                )
                                .await
                            })
                            .await;

                        match res {
                            Ok(msg) => {
                                state.add_log(msg);
                                if let Ok(fresh) = load_fleet(fleet_root).await {
                                    state.reload(fresh);
                                }
                            }
                            Err(e) => state.add_log(format!("failed to create secret: {e}")),
                        }
                    }
                    ActionEffect::ExecuteCreateEnv(form) => {
                        state.add_log(format!("> creating environment '{}'...", form.name));
                        match action::new::create_environment_entity(fleet_root, &form.name).await {
                            Ok(msg) => {
                                state.add_log(msg);
                                if let Ok(fresh) = load_fleet(fleet_root).await {
                                    state.reload(fresh);
                                }
                            }
                            Err(e) => state.add_log(format!("failed to create environment: {e}")),
                        }
                    }
                    ActionEffect::ExecuteSsh {
                        env_name,
                        host_name,
                    } => {
                        let id_ref = state.identity.clone();
                        state.add_log(format!("> connecting to {host_name} via SSH..."));
                        let res = session
                            .suspend(|| async {
                                action::ssh::execute(
                                    fleet_root,
                                    &env_name,
                                    &host_name,
                                    id_ref.as_deref(),
                                )
                                .await
                            })
                            .await;

                        if let Err(e) = res {
                            state.add_log(format!("SSH failed: {e}"));
                        }
                    }
                    ActionEffect::ExecuteLogs {
                        env_name,
                        host_name,
                        service_unit,
                    } => {
                        let id_ref = state.identity.clone();
                        if let Some(ref unit) = service_unit {
                            state.add_log(format!("> journalctl -u {unit} on {host_name}"));
                        } else {
                            state.add_log(format!("> journalctl on {host_name}"));
                        }
                        match action::logs::execute(
                            fleet_root,
                            &env_name,
                            &host_name,
                            service_unit.as_deref(),
                            id_ref.as_deref(),
                        )
                        .await
                        {
                            Ok(lines) => {
                                if lines.is_empty() {
                                    state.add_log("(no journal entries found)".to_string());
                                } else {
                                    for l in lines {
                                        state.add_log(l);
                                    }
                                }
                            }
                            Err(e) => {
                                state.add_log(format!("error: {e}"));
                            }
                        }
                    }
                    ActionEffect::ExecuteCheck => {
                        state.add_log("> rocky check");
                        match action::check::execute(fleet_root).await {
                            Ok(lines) => {
                                for l in lines {
                                    state.add_log(l);
                                }
                            }
                            Err(e) => state.add_log(format!("check failed: {e}")),
                        }
                    }
                    ActionEffect::ExecuteDeploy {
                        env_name,
                        target_host,
                    } => {
                        let id_ref = state.identity.clone();
                        let target_display = target_host
                            .as_deref()
                            .map(|h| format!(" --target {h}"))
                            .unwrap_or_default();
                        state.add_log(format!("> rocky up {env_name}{target_display}"));
                        let res = session
                            .suspend(|| async {
                                let r = action::up::execute(
                                    fleet_root,
                                    &env_name,
                                    target_host.as_deref(),
                                    id_ref.as_deref(),
                                )
                                .await;
                                println!("\nPress Enter to return to Rocky...");
                                let mut buf = String::new();
                                let _ = std::io::stdin().read_line(&mut buf);
                                r
                            })
                            .await;

                        match res {
                            Ok(()) => {
                                state.add_log(format!("deploy completed for '{env_name}'"));
                                if let Ok(fresh) = load_fleet(fleet_root).await {
                                    state.reload(fresh);
                                }
                            }
                            Err(e) => state.add_log(format!("deploy failed: {e}")),
                        }
                    }
                    ActionEffect::Continue => {}
                }
            }
        }
    }

    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::tui::load::{Environment, Fleet, Host};

    fn sample_fleet(mesh_connected: bool) -> Fleet {
        Fleet {
            environments: vec![Environment {
                name: "prod".to_string(),
                domain: "prod.rocky.internal".to_string(),
                hosts: vec![Host {
                    name: "001".to_string(),
                    environment: "prod".to_string(),
                    services: vec!["caddy".to_string()],
                    magic_dns: "prod-001".to_string(),
                    has_disko: true,
                    has_hardware: true,
                    has_key: true,
                }],
                secrets: vec![],
                mesh_connected,
            }],
            services: vec![],
        }
    }

    #[test]
    fn test_classify_key_events_focus_and_navigation() {
        let mut state = TuiState::new(sample_fleet(true), None);

        let event_tab = KeyEvent::new(KeyCode::Tab, KeyModifiers::NONE);
        assert_eq!(classify_key_event(event_tab, &state), KeyAction::CycleFocus);

        let event_backtab = KeyEvent::new(KeyCode::BackTab, KeyModifiers::NONE);
        assert_eq!(
            classify_key_event(event_backtab, &state),
            KeyAction::CycleFocusPrev
        );

        let event_digit = KeyEvent::new(KeyCode::Char('3'), KeyModifiers::NONE);
        assert_eq!(
            classify_key_event(event_digit, &state),
            KeyAction::SetFocus(FocusPanel::Services)
        );

        let event_j = KeyEvent::new(KeyCode::Char('j'), KeyModifiers::NONE);
        assert_eq!(classify_key_event(event_j, &state), KeyAction::MoveDown);

        let event_k = KeyEvent::new(KeyCode::Char('k'), KeyModifiers::NONE);
        assert_eq!(classify_key_event(event_k, &state), KeyAction::MoveUp);

        let event_q = KeyEvent::new(KeyCode::Char('q'), KeyModifiers::NONE);
        assert_eq!(classify_key_event(event_q, &state), KeyAction::Quit);

        let event_help = KeyEvent::new(KeyCode::Char('?'), KeyModifiers::NONE);
        assert_eq!(
            classify_key_event(event_help, &state),
            KeyAction::ToggleHelp
        );

        let event_max = KeyEvent::new(KeyCode::Char('m'), KeyModifiers::NONE);
        assert_eq!(
            classify_key_event(event_max, &state),
            KeyAction::ToggleMaximizeLogs
        );

        state.focus = FocusPanel::Secrets;
        let event_enter = KeyEvent::new(KeyCode::Enter, KeyModifiers::NONE);
        assert_eq!(
            classify_key_event(event_enter, &state),
            KeyAction::PromptIdentity(crate::tui::state::PendingSecretAction::View)
        );

        let event_edit = KeyEvent::new(KeyCode::Char('e'), KeyModifiers::NONE);
        assert_eq!(
            classify_key_event(event_edit, &state),
            KeyAction::PromptIdentity(crate::tui::state::PendingSecretAction::Edit)
        );

        state.identity = Some(PathBuf::from("/test/id"));
        assert_eq!(
            classify_key_event(event_enter, &state),
            KeyAction::ViewSecret
        );
        assert_eq!(classify_key_event(event_edit, &state), KeyAction::Edit);

        state.focus = FocusPanel::Log;
        let event_top = KeyEvent::new(KeyCode::Char('g'), KeyModifiers::NONE);
        assert_eq!(
            classify_key_event(event_top, &state),
            KeyAction::ScrollLogTop
        );

        let event_clear = KeyEvent::new(KeyCode::Char('c'), KeyModifiers::NONE);
        assert_eq!(classify_key_event(event_clear, &state), KeyAction::ClearLog);
    }

    #[test]
    fn test_apply_action_host_modal_flow() {
        let mut state = TuiState::new(sample_fleet(true), None);

        apply_action(&mut state, KeyAction::NewEntity);
        assert_eq!(state.active_popup, ActivePopup::NewEntityMenu);

        apply_action(&mut state, KeyAction::SelectNewEntity(NewEntityType::Host));
        match &state.active_popup {
            ActivePopup::HostModal(f) => {
                assert_eq!(f.environment, "prod");
                assert_eq!(f.provider, "hetzner");
                assert_eq!(f.active_field, 0);
            }
            _ => panic!("expected HostModal"),
        }

        apply_action(&mut state, KeyAction::InputChar('a'));
        apply_action(&mut state, KeyAction::InputChar('p'));
        apply_action(&mut state, KeyAction::InputChar('p'));

        apply_action(&mut state, KeyAction::NextFormField);
        match &state.active_popup {
            ActivePopup::HostModal(f) => assert_eq!(f.active_field, 1),
            _ => panic!("expected HostModal"),
        }

        apply_action(&mut state, KeyAction::NextFormField);
        match &state.active_popup {
            ActivePopup::HostModal(f) => assert_eq!(f.active_field, 2),
            _ => panic!("expected HostModal"),
        }

        apply_action(&mut state, KeyAction::ToggleFormField);
        match &state.active_popup {
            ActivePopup::HostModal(f) => assert_eq!(f.provider, "default"),
            _ => panic!("expected HostModal"),
        }

        let effect = apply_action(&mut state, KeyAction::SubmitForm);
        assert_eq!(
            effect,
            ActionEffect::ExecuteCreateHost(HostForm {
                name: "app".to_string(),
                environment: "prod".to_string(),
                provider: "default".to_string(),
                active_field: 2,
            })
        );
    }
}
