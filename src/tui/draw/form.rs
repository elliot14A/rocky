use ratatui::layout::{Constraint, Direction, Layout, Rect};
use ratatui::style::{Color, Modifier, Style};
use ratatui::text::{Line, Span};
use ratatui::widgets::{Block, BorderType, Borders, Clear, Paragraph, Wrap};
use ratatui::Frame;

use crate::templates::service::ServiceKind;
use crate::tui::state::{ActivePopup, TuiState};

pub fn draw_form_popup(frame: &mut Frame, area: Rect, state: &TuiState) {
    match &state.active_popup {
        ActivePopup::None => {}
        ActivePopup::HelpModal(modal) => {
            super::help::draw_help_modal(frame, area, state, modal);
        }
        ActivePopup::SecretViewer(viewer) => {
            super::viewer::draw_secret_viewer(frame, area, viewer);
        }
        ActivePopup::IdentityModal(modal) => {
            let popup_area = centered_rect(60, 25, area);
            frame.render_widget(Clear, popup_area);

            let block = Block::default()
                .borders(Borders::ALL)
                .border_type(BorderType::Rounded)
                .border_style(Style::default().fg(Color::Cyan))
                .title(" Set Identity Key (-i) ")
                .title_bottom(Line::from(vec![
                    Span::styled(
                        " [Enter] ",
                        Style::default()
                            .fg(Color::Cyan)
                            .add_modifier(Modifier::BOLD),
                    ),
                    Span::raw("Save & Continue  "),
                    Span::styled(" [Esc] ", Style::default().fg(Color::DarkGray)),
                    Span::raw("Cancel"),
                ]));

            let inner = block.inner(popup_area);
            frame.render_widget(block, popup_area);

            let mut lines = vec![
                Line::raw(""),
                Line::styled(
                    " An identity key is required to decrypt/edit secrets.",
                    Style::default().fg(Color::White),
                ),
                Line::styled(
                    " Enter path to private key (e.g. ~/.ssh/id_ed25519):",
                    Style::default().fg(Color::DarkGray),
                ),
                Line::raw(""),
                Line::from(vec![
                    Span::styled(
                        " > ",
                        Style::default()
                            .fg(Color::Cyan)
                            .add_modifier(Modifier::BOLD),
                    ),
                    Span::styled(&modal.input, Style::default().fg(Color::White)),
                    Span::styled("█", Style::default().fg(Color::Cyan)),
                ]),
            ];

            if let Some(ref err) = modal.error {
                lines.push(Line::raw(""));
                lines.push(Line::from(vec![
                    Span::styled(
                        " Error: ",
                        Style::default().fg(Color::Red).add_modifier(Modifier::BOLD),
                    ),
                    Span::styled(err, Style::default().fg(Color::Red)),
                ]));
            }

            frame.render_widget(Paragraph::new(lines), inner);
        }
        ActivePopup::NewEntityMenu => {
            let popup_area = centered_rect(45, 30, area);
            frame.render_widget(Clear, popup_area);

            let block = Block::default()
                .borders(Borders::ALL)
                .border_type(BorderType::Rounded)
                .border_style(Style::default().fg(Color::Cyan))
                .title(" Create Entity ");
            let inner = block.inner(popup_area);
            frame.render_widget(block, popup_area);

            let text = vec![
                Line::raw(""),
                Line::from(vec![
                    Span::styled(
                        " [1] ",
                        Style::default()
                            .fg(Color::Cyan)
                            .add_modifier(Modifier::BOLD),
                    ),
                    Span::raw("New Environment"),
                ]),
                Line::from(vec![
                    Span::styled(
                        " [2] ",
                        Style::default()
                            .fg(Color::Cyan)
                            .add_modifier(Modifier::BOLD),
                    ),
                    Span::raw("New Host"),
                ]),
                Line::from(vec![
                    Span::styled(
                        " [3] ",
                        Style::default()
                            .fg(Color::Cyan)
                            .add_modifier(Modifier::BOLD),
                    ),
                    Span::raw("New Service"),
                ]),
                Line::from(vec![
                    Span::styled(
                        " [4] ",
                        Style::default()
                            .fg(Color::Cyan)
                            .add_modifier(Modifier::BOLD),
                    ),
                    Span::raw("New Secret"),
                ]),
                Line::raw(""),
                Line::from(vec![
                    Span::styled(" [Esc] ", Style::default().fg(Color::DarkGray)),
                    Span::raw("Cancel"),
                ]),
            ];

            frame.render_widget(Paragraph::new(text), inner);
        }
        ActivePopup::HostModal(form) => {
            let popup_area = centered_rect(55, 35, area);
            frame.render_widget(Clear, popup_area);

            let block = Block::default()
                .borders(Borders::ALL)
                .border_type(BorderType::Rounded)
                .border_style(Style::default().fg(Color::Cyan))
                .title(" New Host ");
            let inner = block.inner(popup_area);
            frame.render_widget(block, popup_area);

            let field_style = |idx| {
                if form.active_field == idx {
                    Style::default()
                        .bg(Color::Cyan)
                        .fg(Color::Black)
                        .add_modifier(Modifier::BOLD)
                } else {
                    Style::default().fg(Color::White)
                }
            };

            let text = vec![
                Line::raw(""),
                Line::from(vec![
                    Span::styled("  Name:        ", Style::default().fg(Color::DarkGray)),
                    Span::styled(format!(" {} ", form.name), field_style(0)),
                ]),
                Line::from(vec![
                    Span::styled("  Environment: ", Style::default().fg(Color::DarkGray)),
                    Span::styled(format!(" {} ", form.environment), field_style(1)),
                    Span::styled(" (Space to cycle)", Style::default().fg(Color::DarkGray)),
                ]),
                Line::from(vec![
                    Span::styled("  Provider:    ", Style::default().fg(Color::DarkGray)),
                    Span::styled(format!(" {} ", form.provider), field_style(2)),
                    Span::styled(" (Space to cycle)", Style::default().fg(Color::DarkGray)),
                ]),
                Line::raw(""),
                Line::from(vec![
                    Span::styled("  [Tab/Down] ", Style::default().fg(Color::DarkGray)),
                    Span::raw("Next field   "),
                    Span::styled("[Enter] ", Style::default().fg(Color::Green)),
                    Span::raw("Create   "),
                    Span::styled("[Esc] ", Style::default().fg(Color::DarkGray)),
                    Span::raw("Cancel"),
                ]),
            ];

            frame.render_widget(Paragraph::new(text), inner);
        }
        ActivePopup::ServiceModal(form) => {
            let popup_area = centered_rect(60, 42, area);
            frame.render_widget(Clear, popup_area);

            let block = Block::default()
                .borders(Borders::ALL)
                .border_type(BorderType::Rounded)
                .border_style(Style::default().fg(Color::Cyan))
                .title(" New Service ");
            let inner = block.inner(popup_area);
            frame.render_widget(block, popup_area);

            let field_style = |idx| {
                if form.active_field == idx {
                    Style::default()
                        .bg(Color::Cyan)
                        .fg(Color::Black)
                        .add_modifier(Modifier::BOLD)
                } else {
                    Style::default().fg(Color::White)
                }
            };

            let kind_str = match form.kind {
                ServiceKind::Systemd => "systemd",
                ServiceKind::Podman => "podman",
            };
            let host_str = form.host.as_deref().unwrap_or("(auto)");

            let text = vec![
                Line::raw(""),
                Line::from(vec![
                    Span::styled("  Name:        ", Style::default().fg(Color::DarkGray)),
                    Span::styled(format!(" {} ", form.name), field_style(0)),
                ]),
                Line::from(vec![
                    Span::styled("  Kind:        ", Style::default().fg(Color::DarkGray)),
                    Span::styled(format!(" [{kind_str}] "), field_style(1)),
                    Span::styled(" (Space to toggle)", Style::default().fg(Color::DarkGray)),
                ]),
                Line::from(vec![
                    Span::styled("  Target Host: ", Style::default().fg(Color::DarkGray)),
                    Span::styled(format!(" {host_str} "), field_style(2)),
                    Span::styled(" (Space to cycle)", Style::default().fg(Color::DarkGray)),
                ]),
                Line::from(vec![
                    Span::styled("  Port:        ", Style::default().fg(Color::DarkGray)),
                    Span::styled(format!(" {} ", form.port), field_style(3)),
                    Span::styled(" (optional)", Style::default().fg(Color::DarkGray)),
                ]),
                Line::from(vec![
                    Span::styled("  Replicas:    ", Style::default().fg(Color::DarkGray)),
                    Span::styled(format!(" {} ", form.replicas), field_style(4)),
                    Span::styled(
                        " (digits or Space to cycle)",
                        Style::default().fg(Color::DarkGray),
                    ),
                ]),
                Line::raw(""),
                Line::from(vec![
                    Span::styled("  [Tab/Down] ", Style::default().fg(Color::DarkGray)),
                    Span::raw("Next field   "),
                    Span::styled("[Enter] ", Style::default().fg(Color::Green)),
                    Span::raw("Create   "),
                    Span::styled("[Esc] ", Style::default().fg(Color::DarkGray)),
                    Span::raw("Cancel"),
                ]),
            ];

            frame.render_widget(Paragraph::new(text), inner);
        }
        ActivePopup::SecretModal(form) => {
            let popup_area = centered_rect(55, 35, area);
            frame.render_widget(Clear, popup_area);

            let block = Block::default()
                .borders(Borders::ALL)
                .border_type(BorderType::Rounded)
                .border_style(Style::default().fg(Color::Cyan))
                .title(" New Secret ");
            let inner = block.inner(popup_area);
            frame.render_widget(block, popup_area);

            let field_style = |idx| {
                if form.active_field == idx {
                    Style::default()
                        .bg(Color::Cyan)
                        .fg(Color::Black)
                        .add_modifier(Modifier::BOLD)
                } else {
                    Style::default().fg(Color::White)
                }
            };

            let scope_str = if form.all_hosts {
                "All Hosts"
            } else {
                "Scoped"
            };

            let text = vec![
                Line::raw(""),
                Line::from(vec![
                    Span::styled("  Name:        ", Style::default().fg(Color::DarkGray)),
                    Span::styled(format!(" {} ", form.name), field_style(0)),
                ]),
                Line::from(vec![
                    Span::styled("  Environment: ", Style::default().fg(Color::DarkGray)),
                    Span::styled(format!(" {} ", form.environment), field_style(1)),
                    Span::styled(" (Space to cycle)", Style::default().fg(Color::DarkGray)),
                ]),
                Line::from(vec![
                    Span::styled("  Scope:       ", Style::default().fg(Color::DarkGray)),
                    Span::styled(format!(" [{scope_str}] "), field_style(2)),
                    Span::styled(" (Space to toggle)", Style::default().fg(Color::DarkGray)),
                ]),
                Line::raw(""),
                Line::from(vec![
                    Span::styled("  [Enter] ", Style::default().fg(Color::Green)),
                    Span::raw("Create & Edit in $EDITOR   "),
                    Span::styled("[Esc] ", Style::default().fg(Color::DarkGray)),
                    Span::raw("Cancel"),
                ]),
            ];

            frame.render_widget(Paragraph::new(text), inner);
        }
        ActivePopup::EnvModal(form) => {
            let popup_area = centered_rect(50, 22, area);
            frame.render_widget(Clear, popup_area);

            let block = Block::default()
                .borders(Borders::ALL)
                .border_type(BorderType::Rounded)
                .border_style(Style::default().fg(Color::Cyan))
                .title(" New Environment ");
            let inner = block.inner(popup_area);
            frame.render_widget(block, popup_area);

            let text = vec![
                Line::raw(""),
                Line::from(vec![
                    Span::styled("  Name: ", Style::default().fg(Color::DarkGray)),
                    Span::styled(
                        format!(" {} ", form.name),
                        Style::default()
                            .bg(Color::Cyan)
                            .fg(Color::Black)
                            .add_modifier(Modifier::BOLD),
                    ),
                ]),
                Line::raw(""),
                Line::from(vec![
                    Span::styled("  [Enter] ", Style::default().fg(Color::Green)),
                    Span::raw("Create   "),
                    Span::styled("[Esc] ", Style::default().fg(Color::DarkGray)),
                    Span::raw("Cancel"),
                ]),
            ];

            frame.render_widget(Paragraph::new(text), inner);
        }
        ActivePopup::ConfirmJoinMesh { env_name } => {
            let popup_area = centered_rect(50, 20, area);
            frame.render_widget(Clear, popup_area);

            let block = Block::default()
                .borders(Borders::ALL)
                .border_type(BorderType::Rounded)
                .border_style(Style::default().fg(Color::Yellow))
                .title(" Join Fleet Mesh ");
            let inner = block.inner(popup_area);
            frame.render_widget(block, popup_area);

            let text = vec![
                Line::raw(""),
                Line::from(vec![
                    Span::raw("Workstation is not connected to mesh '"),
                    Span::styled(env_name, Style::default().fg(Color::Cyan)),
                    Span::raw("'."),
                ]),
                Line::raw(""),
                Line::from(vec![
                    Span::styled("[Enter/j] ", Style::default().fg(Color::Green)),
                    Span::raw("Join mesh now    "),
                    Span::styled("[Esc] ", Style::default().fg(Color::DarkGray)),
                    Span::raw("Cancel"),
                ]),
            ];

            frame.render_widget(Paragraph::new(text).wrap(Wrap { trim: true }), inner);
        }
        ActivePopup::Error(msg) => {
            let popup_area = centered_rect(50, 25, area);
            frame.render_widget(Clear, popup_area);

            let block = Block::default()
                .borders(Borders::ALL)
                .border_type(BorderType::Rounded)
                .border_style(Style::default().fg(Color::Red))
                .title(" Error ");
            let inner = block.inner(popup_area);
            frame.render_widget(block, popup_area);

            let text = vec![
                Line::raw(""),
                Line::styled(msg, Style::default().fg(Color::Red)),
                Line::raw(""),
                Line::from(vec![
                    Span::styled("[Esc] ", Style::default().fg(Color::DarkGray)),
                    Span::raw("Dismiss"),
                ]),
            ];

            frame.render_widget(Paragraph::new(text).wrap(Wrap { trim: true }), inner);
        }
    }
}

pub fn centered_rect(percent_x: u16, percent_y: u16, area: Rect) -> Rect {
    let popup_layout = Layout::default()
        .direction(Direction::Vertical)
        .constraints([
            Constraint::Percentage((100 - percent_y) / 2),
            Constraint::Percentage(percent_y),
            Constraint::Percentage((100 - percent_y) / 2),
        ])
        .split(area);

    Layout::default()
        .direction(Direction::Horizontal)
        .constraints([
            Constraint::Percentage((100 - percent_x) / 2),
            Constraint::Percentage(percent_x),
            Constraint::Percentage((100 - percent_x) / 2),
        ])
        .split(popup_layout[1])[1]
}
