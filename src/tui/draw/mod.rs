pub mod details;
pub mod envs_hosts;
pub mod footer;
pub mod form;
pub mod help;
pub mod log;
pub mod secrets;
pub mod services;
pub mod status;
pub mod viewer;

use ratatui::layout::{Constraint, Direction, Layout};
use ratatui::Frame;

use self::details::draw_details;
use self::envs_hosts::draw_envs_hosts;
use self::footer::draw_footer;
use self::form::draw_form_popup;
use self::log::draw_command_log;
use self::secrets::draw_secrets;
use self::services::draw_services;
use self::status::draw_status;
use super::state::TuiState;

pub fn draw(frame: &mut Frame, state: &TuiState) {
    let root = Layout::default()
        .direction(Direction::Vertical)
        .constraints([Constraint::Min(10), Constraint::Length(1)])
        .split(frame.area());

    if state.logs_maximized {
        draw_command_log(frame, root[0], state);
        draw_footer(frame, root[1], state);
        draw_form_popup(frame, frame.area(), state);
        return;
    }

    let body = Layout::default()
        .direction(Direction::Horizontal)
        .constraints([Constraint::Percentage(38), Constraint::Percentage(62)])
        .split(root[0]);

    let left_chunks = Layout::default()
        .direction(Direction::Vertical)
        .constraints([
            Constraint::Length(3),
            Constraint::Percentage(45),
            Constraint::Percentage(25),
            Constraint::Percentage(30),
        ])
        .split(body[0]);

    draw_status(frame, left_chunks[0], state);
    draw_envs_hosts(frame, left_chunks[1], state);
    draw_services(frame, left_chunks[2], state);
    draw_secrets(frame, left_chunks[3], state);

    let right_chunks = Layout::default()
        .direction(Direction::Vertical)
        .constraints([Constraint::Percentage(60), Constraint::Percentage(40)])
        .split(body[1]);

    draw_details(frame, right_chunks[0], state);
    draw_command_log(frame, right_chunks[1], state);

    draw_footer(frame, root[1], state);
    draw_form_popup(frame, frame.area(), state);
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::tui::load::{Environment, Fleet, Host, Secret, Service};
    use ratatui::backend::TestBackend;
    use ratatui::Terminal;

    fn sample_fleet() -> Fleet {
        Fleet {
            environments: vec![Environment {
                name: "prod".to_string(),
                domain: "prod.rocky.internal".to_string(),
                hosts: vec![
                    Host {
                        name: "001".to_string(),
                        environment: "prod".to_string(),
                        services: vec!["caddy".to_string(), "postgres".to_string()],
                        magic_dns: "prod-001".to_string(),
                        has_disko: true,
                        has_hardware: true,
                        has_key: true,
                    },
                    Host {
                        name: "002".to_string(),
                        environment: "prod".to_string(),
                        services: vec![],
                        magic_dns: "prod-002".to_string(),
                        has_disko: true,
                        has_hardware: true,
                        has_key: true,
                    },
                ],
                secrets: vec![Secret {
                    name: "acme.age".to_string(),
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
                hosts: vec!["prod:001".to_string()],
                secrets: vec![],
            }],
        }
    }

    #[test]
    fn test_draw_renders_five_panels() {
        let backend = TestBackend::new(120, 35);
        let mut terminal = Terminal::new(backend).unwrap();
        let state = TuiState::new(sample_fleet(), None);

        terminal.draw(|f| draw(f, &state)).unwrap();
        let buffer = terminal.backend().buffer();
        let rendered = format!("{:?}", buffer);

        assert!(rendered.contains("[1] Status"));
        assert!(rendered.contains("[2] Environments & Hosts"));
        assert!(rendered.contains("[3] Services"));
        assert!(rendered.contains("[4] Secrets"));
        assert!(rendered.contains("Command Log"));
        assert!(rendered.contains("001"));
        assert!(rendered.contains("002"));
        assert!(rendered.contains("caddy"));
    }

    #[test]
    fn test_draw_renders_form_popups() {
        use crate::templates::service::ServiceKind;
        use crate::tui::state::{ActivePopup, FocusPanel, HostForm, ServiceForm};

        let backend = TestBackend::new(120, 35);
        let mut terminal = Terminal::new(backend).unwrap();
        let mut state = TuiState::new(sample_fleet(), None);

        state.active_popup = ActivePopup::HostModal(HostForm {
            name: "003".to_string(),
            environment: "prod".to_string(),
            provider: "hetzner".to_string(),
            active_field: 0,
        });
        terminal.draw(|f| draw(f, &state)).unwrap();
        let rendered = format!("{:?}", terminal.backend().buffer());
        assert!(rendered.contains("New Host"));

        state.active_popup = ActivePopup::ServiceModal(ServiceForm {
            name: "web".to_string(),
            kind: ServiceKind::Systemd,
            host: Some("001".to_string()),
            port: "8080".to_string(),
            replicas: 1,
            active_field: 0,
        });
        terminal.draw(|f| draw(f, &state)).unwrap();
        let rendered = format!("{:?}", terminal.backend().buffer());
        assert!(rendered.contains("New Service"));

        state.active_popup = ActivePopup::None;
        state.set_focus(FocusPanel::Details);
        state.add_log("command completed successfully");
        terminal.draw(|f| draw(f, &state)).unwrap();
        let rendered = format!("{:?}", terminal.backend().buffer());
        assert!(rendered.contains("command completed successfully"));
    }
}
