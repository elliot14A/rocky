use ratatui::layout::Rect;
use ratatui::style::{Color, Modifier, Style};
use ratatui::text::{Line, Span};
use ratatui::widgets::{Block, BorderType, Borders, Paragraph};
use ratatui::Frame;

use crate::tui::state::{FocusPanel, TreeItem, TuiState};

pub fn draw_envs_hosts(frame: &mut Frame, area: Rect, state: &TuiState) {
    let is_focused = state.focus == FocusPanel::Environments;
    let border_color = if is_focused {
        Color::Cyan
    } else {
        Color::DarkGray
    };

    let items = state.tree_items();
    let current_idx = if items.is_empty() {
        0
    } else {
        state.env_host_index + 1
    };
    let total_count = items.len();

    let block = Block::default()
        .borders(Borders::ALL)
        .border_type(BorderType::Rounded)
        .border_style(Style::default().fg(border_color))
        .title(" [2] Environments & Hosts ")
        .title_bottom(Line::from(vec![Span::styled(
            format!(" {current_idx}/{total_count} "),
            Style::default().fg(Color::DarkGray),
        )]));

    let inner = block.inner(area);
    frame.render_widget(block, area);

    let max_rows = inner.height as usize;
    let mut scroll = state.env_scroll.get();
    crate::tui::state::adjust_scroll(state.env_host_index, &mut scroll, max_rows);
    state.env_scroll.set(scroll);

    let visible_items: Vec<Line> = items
        .iter()
        .enumerate()
        .skip(scroll)
        .take(max_rows)
        .map(|(idx, item)| {
            let is_selected = idx == state.env_host_index;
            match item {
                TreeItem::Environment {
                    name,
                    mesh_connected: _,
                    expanded,
                } => {
                    let arrow = if *expanded { "▼ " } else { "▶ " };
                    let style = if is_selected {
                        Style::default()
                            .fg(Color::Cyan)
                            .add_modifier(Modifier::BOLD)
                    } else {
                        Style::default()
                            .fg(Color::White)
                            .add_modifier(Modifier::BOLD)
                    };

                    Line::from(vec![
                        Span::styled(arrow, Style::default().fg(Color::White)),
                        Span::styled(name, style),
                    ])
                }
                TreeItem::Host {
                    name,
                    env_name,
                    has_services,
                    expanded,
                } => {
                    let env_mesh = state
                        .fleet
                        .environments
                        .iter()
                        .find(|e| &e.name == env_name)
                        .map(|e| e.mesh_connected)
                        .unwrap_or(false);

                    let dot_color = if env_mesh {
                        Color::Green
                    } else {
                        Color::DarkGray
                    };

                    let arrow = if *has_services {
                        if *expanded {
                            "▼ "
                        } else {
                            "▶ "
                        }
                    } else {
                        "  "
                    };

                    if is_selected {
                        Line::from(vec![
                            Span::raw("  "),
                            Span::styled(arrow, Style::default().fg(Color::White)),
                            Span::styled("● ", Style::default().fg(dot_color)),
                            Span::styled(
                                name,
                                Style::default()
                                    .bg(Color::Cyan)
                                    .fg(Color::Black)
                                    .add_modifier(Modifier::BOLD),
                            ),
                        ])
                    } else {
                        Line::from(vec![
                            Span::raw("  "),
                            Span::styled(arrow, Style::default().fg(Color::DarkGray)),
                            Span::styled("● ", Style::default().fg(dot_color)),
                            Span::styled(name, Style::default().fg(Color::White)),
                        ])
                    }
                }
                TreeItem::HostService { name, is_last, .. } => {
                    let branch = if *is_last { "└─ " } else { "├─ " };
                    if is_selected {
                        Line::from(vec![
                            Span::raw("    "),
                            Span::styled(branch, Style::default().fg(Color::DarkGray)),
                            Span::styled(
                                name,
                                Style::default()
                                    .bg(Color::Cyan)
                                    .fg(Color::Black)
                                    .add_modifier(Modifier::BOLD),
                            ),
                        ])
                    } else {
                        Line::from(vec![
                            Span::raw("    "),
                            Span::styled(branch, Style::default().fg(Color::DarkGray)),
                            Span::styled(name, Style::default().fg(Color::White)),
                        ])
                    }
                }
            }
        })
        .collect();

    frame.render_widget(Paragraph::new(visible_items), inner);
}
