use ratatui::layout::Rect;
use ratatui::style::{Color, Modifier, Style};
use ratatui::text::{Line, Span};
use ratatui::widgets::{Block, BorderType, Borders, Paragraph};
use ratatui::Frame;

use crate::tui::state::{FocusPanel, TreeItem, TuiState};

pub fn draw_details(frame: &mut Frame, area: Rect, state: &TuiState) {
    let is_focused = state.focus == FocusPanel::Details;
    let border_color = if is_focused {
        Color::Cyan
    } else {
        Color::DarkGray
    };

    let (title, lines) = match state.focus {
        FocusPanel::Services => {
            if let Some(svc) = state.selected_service() {
                (
                    format!(" [0] Service: {} ", svc.name),
                    render_service_details(svc),
                )
            } else {
                (
                    " [0] Service ".to_string(),
                    vec![Line::raw("No service selected")],
                )
            }
        }
        FocusPanel::Secrets => {
            if let Some(sec) = state.selected_secret() {
                (
                    format!(" [0] Secret: {} ", sec.name),
                    render_secret_details(sec),
                )
            } else {
                (
                    " [0] Secret ".to_string(),
                    vec![Line::raw("No secret selected")],
                )
            }
        }
        _ => match state.selected_tree_item() {
            Some(TreeItem::Host { name, env_name, .. }) => {
                let env = state
                    .fleet
                    .environments
                    .iter()
                    .find(|e| &e.name == env_name);
                let host = env.and_then(|e| e.hosts.iter().find(|h| &h.name == name));

                if let (Some(h), Some(e)) = (host, env) {
                    (
                        format!(" [0] Host: {name} "),
                        render_host_details(h, &e.domain),
                    )
                } else if let Some(h) = host {
                    (format!(" [0] Host: {name} "), render_host_details(h, ""))
                } else {
                    (format!(" [0] Host: {name} "), vec![])
                }
            }
            Some(TreeItem::HostService {
                name,
                host_name,
                env_name,
                ..
            }) => {
                let svc = state.fleet.services.iter().find(|s| &s.name == name);
                if let Some(s) = svc {
                    (
                        format!(" [0] Service: {name} ({host_name}) "),
                        render_service_on_host_details(s, host_name, env_name),
                    )
                } else {
                    (format!(" [0] Service: {name} "), vec![])
                }
            }
            Some(TreeItem::Environment { name, .. }) => {
                let env = state.fleet.environments.iter().find(|e| &e.name == name);
                if let Some(e) = env {
                    (
                        format!(" [0] Environment: {name} "),
                        render_environment_details(e),
                    )
                } else {
                    (format!(" [0] Environment: {name} "), vec![])
                }
            }
            None => (
                " [0] Inspector ".to_string(),
                vec![Line::raw("No selection")],
            ),
        },
    };

    let block = Block::default()
        .borders(Borders::ALL)
        .border_type(BorderType::Rounded)
        .border_style(Style::default().fg(border_color))
        .title(title)
        .title_bottom(Line::from(vec![Span::raw(" 🔍 ")]));

    let inner = block.inner(area);
    frame.render_widget(block, area);
    frame.render_widget(Paragraph::new(lines), inner);
}

fn render_host_details<'a>(host: &'a crate::tui::load::Host, env_domain: &'a str) -> Vec<Line<'a>> {
    let mut lines = Vec::new();

    lines.push(Line::from(vec![
        Span::styled("Host:           ", Style::default().fg(Color::DarkGray)),
        Span::styled(&host.name, Style::default().add_modifier(Modifier::BOLD)),
    ]));
    lines.push(Line::from(vec![
        Span::styled("Environment:    ", Style::default().fg(Color::DarkGray)),
        Span::raw(&host.environment),
    ]));
    lines.push(Line::from(vec![
        Span::styled("MagicDNS:       ", Style::default().fg(Color::DarkGray)),
        Span::styled(&host.magic_dns, Style::default().fg(Color::Cyan)),
    ]));
    let host_domain = if env_domain.is_empty() {
        format!("{}.{}.rocky.internal", host.name, host.environment)
    } else {
        format!("{}.{env_domain}", host.name)
    };
    lines.push(Line::from(vec![
        Span::styled("Domain:         ", Style::default().fg(Color::DarkGray)),
        Span::raw(host_domain),
    ]));

    let (key_status, key_style) = if host.has_key {
        (
            format!("registered (environments/{}/keys.nix)", host.environment),
            Style::default().fg(Color::Green),
        )
    } else {
        ("missing".to_string(), Style::default().fg(Color::Yellow))
    };
    lines.push(Line::from(vec![
        Span::styled("SSH Host Key:   ", Style::default().fg(Color::DarkGray)),
        Span::styled(key_status, key_style),
    ]));

    let (disko_status, disko_style) = if host.has_disko {
        (
            format!("present (hosts/{}/disko.nix)", host.name),
            Style::default().fg(Color::Green),
        )
    } else {
        ("missing".to_string(), Style::default().fg(Color::Yellow))
    };
    lines.push(Line::from(vec![
        Span::styled("Disko:          ", Style::default().fg(Color::DarkGray)),
        Span::styled(disko_status, disko_style),
    ]));

    let (hw_status, hw_style) = if host.has_hardware {
        (
            format!("present (hosts/{}/hardware.nix)", host.name),
            Style::default().fg(Color::Green),
        )
    } else {
        ("missing".to_string(), Style::default().fg(Color::Yellow))
    };
    lines.push(Line::from(vec![
        Span::styled("Hardware:       ", Style::default().fg(Color::DarkGray)),
        Span::styled(hw_status, hw_style),
    ]));

    lines.push(Line::raw(""));
    lines.push(Line::styled(
        format!("Assigned Services ({}):", host.services.len()),
        Style::default().add_modifier(Modifier::BOLD),
    ));

    if host.services.is_empty() {
        lines.push(Line::styled(
            "  • (none)",
            Style::default().fg(Color::DarkGray),
        ));
    } else {
        for svc in &host.services {
            lines.push(Line::from(vec![
                Span::raw("  "),
                Span::styled("• ", Style::default().fg(Color::Cyan)),
                Span::styled(svc, Style::default().fg(Color::Cyan)),
            ]));
        }
    }

    lines
}

fn render_environment_details(env: &crate::tui::load::Environment) -> Vec<Line<'_>> {
    let mut lines = Vec::new();
    lines.push(Line::from(vec![
        Span::styled("Environment:    ", Style::default().fg(Color::DarkGray)),
        Span::styled(&env.name, Style::default().add_modifier(Modifier::BOLD)),
    ]));
    lines.push(Line::from(vec![
        Span::styled("Domain:         ", Style::default().fg(Color::DarkGray)),
        Span::raw(&env.domain),
    ]));

    let (mesh_status, mesh_color) = if env.mesh_connected {
        ("Connected", Color::Green)
    } else {
        ("Disconnected", Color::Yellow)
    };
    lines.push(Line::from(vec![
        Span::styled("Mesh:           ", Style::default().fg(Color::DarkGray)),
        Span::styled(mesh_status, Style::default().fg(mesh_color)),
    ]));

    lines.push(Line::raw(""));
    lines.push(Line::styled(
        format!("Declared Hosts ({}):", env.hosts.len()),
        Style::default().add_modifier(Modifier::BOLD),
    ));
    for h in &env.hosts {
        lines.push(Line::from(vec![
            Span::raw("  • "),
            Span::styled(&h.name, Style::default().fg(Color::White)),
        ]));
    }

    lines.push(Line::raw(""));
    lines.push(Line::styled(
        format!("Declared Secrets ({}):", env.secrets.len()),
        Style::default().add_modifier(Modifier::BOLD),
    ));
    for s in &env.secrets {
        lines.push(Line::from(vec![
            Span::raw("  • "),
            Span::styled(&s.name, Style::default().fg(Color::White)),
            Span::styled(
                format!(" ({})", s.recipients),
                Style::default().fg(Color::DarkGray),
            ),
        ]));
    }

    lines
}

fn render_service_details(svc: &crate::tui::load::Service) -> Vec<Line<'_>> {
    let mut lines = vec![
        Line::from(vec![
            Span::styled("Service:        ", Style::default().fg(Color::DarkGray)),
            Span::styled(&svc.name, Style::default().add_modifier(Modifier::BOLD)),
        ]),
        Line::from(vec![
            Span::styled("Workload:       ", Style::default().fg(Color::DarkGray)),
            Span::styled(
                format!("[{}]", svc.kind),
                Style::default()
                    .fg(Color::Cyan)
                    .add_modifier(Modifier::BOLD),
            ),
        ]),
        Line::from(vec![
            Span::styled("Target Unit:    ", Style::default().fg(Color::DarkGray)),
            Span::raw(&svc.target),
        ]),
        Line::from(vec![
            Span::styled("Definition:     ", Style::default().fg(Color::DarkGray)),
            Span::styled(
                format!("services/{}.nix", svc.name),
                Style::default().fg(Color::Green),
            ),
        ]),
    ];

    lines.push(Line::raw(""));
    lines.push(Line::styled(
        format!("Configured Environments ({}):", svc.environments.len()),
        Style::default().add_modifier(Modifier::BOLD),
    ));
    if svc.environments.is_empty() {
        lines.push(Line::styled(
            "  (none - unassigned)",
            Style::default().fg(Color::DarkGray),
        ));
    } else {
        for env in &svc.environments {
            let assigned_hosts: Vec<String> = svc
                .hosts
                .iter()
                .filter(|h| h.starts_with(&format!("{env}:")))
                .map(|h| h.strip_prefix(&format!("{env}:")).unwrap_or(h).to_string())
                .collect();
            if assigned_hosts.is_empty() {
                lines.push(Line::from(vec![
                    Span::raw("  • "),
                    Span::styled(env, Style::default().fg(Color::White)),
                ]));
            } else {
                lines.push(Line::from(vec![
                    Span::raw("  • "),
                    Span::styled(env, Style::default().fg(Color::White)),
                    Span::styled(
                        format!(" (hosts: {})", assigned_hosts.join(", ")),
                        Style::default().fg(Color::DarkGray),
                    ),
                ]));
            }
        }
    }

    lines
}

fn render_secret_details(sec: &crate::tui::load::Secret) -> Vec<Line<'_>> {
    vec![
        Line::from(vec![
            Span::styled("Secret:         ", Style::default().fg(Color::DarkGray)),
            Span::styled(&sec.name, Style::default().add_modifier(Modifier::BOLD)),
        ]),
        Line::from(vec![
            Span::styled("Environment:    ", Style::default().fg(Color::DarkGray)),
            Span::raw(&sec.environment),
        ]),
        Line::from(vec![
            Span::styled("Key Recipients: ", Style::default().fg(Color::DarkGray)),
            Span::styled(&sec.recipients, Style::default().fg(Color::Green)),
        ]),
    ]
}

fn render_service_on_host_details(
    svc: &crate::tui::load::Service,
    host_name: &str,
    env_name: &str,
) -> Vec<Line<'static>> {
    let mut lines = vec![
        Line::from(vec![
            Span::styled("Service:        ", Style::default().fg(Color::DarkGray)),
            Span::styled(
                svc.name.clone(),
                Style::default().add_modifier(Modifier::BOLD),
            ),
        ]),
        Line::from(vec![
            Span::styled("Host:           ", Style::default().fg(Color::DarkGray)),
            Span::styled(host_name.to_string(), Style::default().fg(Color::Cyan)),
        ]),
        Line::from(vec![
            Span::styled("Environment:    ", Style::default().fg(Color::DarkGray)),
            Span::raw(env_name.to_string()),
        ]),
        Line::from(vec![
            Span::styled("Workload:       ", Style::default().fg(Color::DarkGray)),
            Span::styled(
                format!("[{}]", svc.kind),
                Style::default()
                    .fg(Color::Cyan)
                    .add_modifier(Modifier::BOLD),
            ),
        ]),
        Line::from(vec![
            Span::styled("Replicas:       ", Style::default().fg(Color::DarkGray)),
            Span::styled(
                if svc.replicas > 1 {
                    format!("{} replicas", svc.replicas)
                } else {
                    "1 replica (singleton)".to_string()
                },
                Style::default()
                    .fg(if svc.replicas > 1 {
                        Color::Green
                    } else {
                        Color::White
                    })
                    .add_modifier(Modifier::BOLD),
            ),
        ]),
    ];

    if let Some(port) = svc.base_port {
        lines.push(Line::from(vec![
            Span::styled("Base Port:      ", Style::default().fg(Color::DarkGray)),
            Span::styled(format!("{port}"), Style::default().fg(Color::Yellow)),
        ]));
    }

    lines.push(Line::from(vec![
        Span::styled("Target Unit:    ", Style::default().fg(Color::DarkGray)),
        Span::raw(svc.target.clone()),
    ]));
    lines.push(Line::from(vec![
        Span::styled("Definition:     ", Style::default().fg(Color::DarkGray)),
        Span::styled(
            format!("services/{}.nix", svc.name),
            Style::default().fg(Color::Green),
        ),
    ]));

    lines
}
