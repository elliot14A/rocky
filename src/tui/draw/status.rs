use ratatui::layout::Rect;
use ratatui::style::{Color, Modifier, Style};
use ratatui::text::{Line, Span};
use ratatui::widgets::{Block, BorderType, Borders, Paragraph};
use ratatui::Frame;

use crate::tui::state::{FocusPanel, TuiState};

pub fn draw_status(frame: &mut Frame, area: Rect, state: &TuiState) {
    let is_focused = state.focus == FocusPanel::Status;
    let border_color = if is_focused {
        Color::Cyan
    } else {
        Color::DarkGray
    };

    let block = Block::default()
        .borders(Borders::ALL)
        .border_type(BorderType::Rounded)
        .border_style(Style::default().fg(border_color))
        .title(" [1] Status ");

    let total_envs = state.fleet.environments.len();
    let total_hosts: usize = state.fleet.environments.iter().map(|e| e.hosts.len()).sum();
    let total_secrets: usize = state
        .fleet
        .environments
        .iter()
        .map(|e| e.secrets.len())
        .sum();

    let content = Line::from(vec![
        Span::styled("✓ ", Style::default().fg(Color::Green)),
        Span::styled(
            "rocky",
            Style::default()
                .fg(Color::White)
                .add_modifier(Modifier::BOLD),
        ),
        Span::styled("  •  ", Style::default().fg(Color::DarkGray)),
        Span::styled(
            format!("{total_envs} envs, {total_hosts} hosts, {total_secrets} secrets"),
            Style::default().fg(Color::White),
        ),
    ]);

    let paragraph = Paragraph::new(content).block(block);
    frame.render_widget(paragraph, area);
}
