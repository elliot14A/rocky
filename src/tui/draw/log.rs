use ratatui::layout::Rect;
use ratatui::style::{Color, Style};
use ratatui::text::{Line, Span};
use ratatui::widgets::{Block, BorderType, Borders, Paragraph};
use ratatui::Frame;

use crate::tui::state::{FocusPanel, TuiState};

pub fn draw_command_log(frame: &mut Frame, area: Rect, state: &TuiState) {
    let is_focused = state.focus == FocusPanel::Log;
    let border_color = if is_focused {
        Color::Cyan
    } else {
        Color::DarkGray
    };

    let title = if state.logs_maximized {
        " [5] Command Log (Maximized - press 'm' or Esc to restore) "
    } else if is_focused {
        " [5] Command Log "
    } else {
        " Command Log "
    };

    let total = state.command_log.len();
    let inner_height = area.height.saturating_sub(2) as usize;
    let base_skip = total.saturating_sub(inner_height);
    let effective_scroll = state.log_scroll.min(base_skip);

    let mut bottom_spans = Vec::new();
    if effective_scroll > 0 {
        bottom_spans.push(Span::styled(
            format!(" [-{effective_scroll} lines] "),
            Style::default().fg(Color::Yellow),
        ));
    }
    if is_focused && !state.logs_maximized {
        bottom_spans.push(Span::styled(
            " [m: Maximize] ",
            Style::default().fg(Color::DarkGray),
        ));
    } else if state.logs_maximized {
        bottom_spans.push(Span::styled(
            " [m/Esc: Restore] ",
            Style::default().fg(Color::DarkGray),
        ));
    }

    let mut block = Block::default()
        .borders(Borders::ALL)
        .border_type(BorderType::Rounded)
        .border_style(Style::default().fg(border_color))
        .title(title);

    if !bottom_spans.is_empty() {
        block = block.title_bottom(Line::from(bottom_spans));
    }

    let inner = block.inner(area);
    frame.render_widget(block, area);

    let inner_rows = inner.height as usize;
    let lines: Vec<Line> = if state.command_log.is_empty() {
        vec![
            Line::styled(
                "> rocky fleet cockpit ready",
                Style::default().fg(Color::DarkGray),
            ),
            Line::styled(
                "press 'u' to deploy, 'c' to run check, 'j' to join mesh, '?' for help",
                Style::default().fg(Color::DarkGray),
            ),
        ]
    } else {
        let skip = base_skip.saturating_sub(effective_scroll);

        state
            .command_log
            .iter()
            .skip(skip)
            .take(inner_rows)
            .map(|l| {
                if let Some(stripped) = l.strip_prefix('>') {
                    Line::from(vec![
                        Span::styled("> ", Style::default().fg(Color::Cyan)),
                        Span::styled(stripped, Style::default().fg(Color::White)),
                    ])
                } else if l.contains("ok") || l.contains("passed") || l.contains("success") {
                    Line::styled(l, Style::default().fg(Color::Green))
                } else if l.contains("error") || l.contains("failed") {
                    Line::styled(l, Style::default().fg(Color::Red))
                } else {
                    Line::styled(l, Style::default().fg(Color::White))
                }
            })
            .collect()
    };

    frame.render_widget(Paragraph::new(lines), inner);
}
