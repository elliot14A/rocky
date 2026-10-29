use ratatui::layout::Rect;
use ratatui::style::{Color, Modifier, Style};
use ratatui::text::{Line, Span};
use ratatui::widgets::{Block, BorderType, Borders, Paragraph};
use ratatui::Frame;

use crate::tui::state::{FocusPanel, TuiState};

pub fn draw_services(frame: &mut Frame, area: Rect, state: &TuiState) {
    let is_focused = state.focus == FocusPanel::Services;
    let border_color = if is_focused {
        Color::Cyan
    } else {
        Color::DarkGray
    };

    let services = &state.fleet.services;
    let current_idx = if services.is_empty() {
        0
    } else {
        state.service_index + 1
    };
    let total_count = services.len();

    let block = Block::default()
        .borders(Borders::ALL)
        .border_type(BorderType::Rounded)
        .border_style(Style::default().fg(border_color))
        .title(" [3] Services ")
        .title_bottom(Line::from(vec![Span::styled(
            format!(" {current_idx}/{total_count} "),
            Style::default().fg(Color::DarkGray),
        )]));

    let inner = block.inner(area);
    frame.render_widget(block, area);

    let max_rows = inner.height as usize;
    let mut scroll = state.service_scroll.get();
    crate::tui::state::adjust_scroll(state.service_index, &mut scroll, max_rows);
    state.service_scroll.set(scroll);

    let items: Vec<Line> = services
        .iter()
        .enumerate()
        .skip(scroll)
        .take(max_rows)
        .map(|(idx, svc)| {
            let is_selected = is_focused && idx == state.service_index;
            if is_selected {
                Line::from(vec![Span::styled(
                    format!(" {}", svc.name),
                    Style::default()
                        .bg(Color::Cyan)
                        .fg(Color::Black)
                        .add_modifier(Modifier::BOLD),
                )])
            } else {
                Line::from(vec![Span::styled(
                    format!(" {}", svc.name),
                    Style::default().fg(Color::White),
                )])
            }
        })
        .collect();

    frame.render_widget(Paragraph::new(items), inner);
}
