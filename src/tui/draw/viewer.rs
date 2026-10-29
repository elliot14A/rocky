use ratatui::layout::Rect;
use ratatui::style::{Color, Modifier, Style};
use ratatui::text::{Line, Span};
use ratatui::widgets::{Block, BorderType, Borders, Clear, Paragraph};
use ratatui::Frame;

use crate::tui::draw::form::centered_rect;
use crate::tui::state::SecretViewerState;

pub fn draw_secret_viewer(frame: &mut Frame, area: Rect, viewer: &SecretViewerState) {
    let popup_area = centered_rect(70, 65, area);
    frame.render_widget(Clear, popup_area);

    let block = Block::default()
        .borders(Borders::ALL)
        .border_type(BorderType::Rounded)
        .border_style(Style::default().fg(Color::Cyan))
        .title(format!(
            " Secret: {} ({}) ",
            viewer.name, viewer.environment
        ))
        .title_bottom(Line::from(vec![
            Span::styled(" [Esc/q/Enter] ", Style::default().fg(Color::DarkGray)),
            Span::raw("Close  "),
            Span::styled(
                " [e] ",
                Style::default()
                    .fg(Color::Cyan)
                    .add_modifier(Modifier::BOLD),
            ),
            Span::raw("Edit in $EDITOR  "),
            Span::styled(" [j/k] ", Style::default().fg(Color::DarkGray)),
            Span::raw("Scroll"),
        ]));

    let inner = block.inner(popup_area);
    frame.render_widget(block, popup_area);

    let max_rows = inner.height as usize;
    let lines: Vec<&str> = viewer.content.lines().collect();
    let total = lines.len();

    let skip = viewer.scroll.min(total.saturating_sub(max_rows));
    let display_lines: Vec<Line> = lines
        .iter()
        .skip(skip)
        .take(max_rows)
        .enumerate()
        .map(|(idx, line)| {
            let line_num = skip + idx + 1;
            let num_span = Span::styled(
                format!("{line_num:>3} | "),
                Style::default().fg(Color::DarkGray),
            );
            if let Some((k, v)) = line.split_once('=') {
                Line::from(vec![
                    num_span,
                    Span::styled(
                        k.to_string(),
                        Style::default()
                            .fg(Color::Cyan)
                            .add_modifier(Modifier::BOLD),
                    ),
                    Span::styled("=", Style::default().fg(Color::DarkGray)),
                    Span::styled(v.to_string(), Style::default().fg(Color::Green)),
                ])
            } else {
                Line::from(vec![
                    num_span,
                    Span::styled(line.to_string(), Style::default().fg(Color::White)),
                ])
            }
        })
        .collect();

    frame.render_widget(Paragraph::new(display_lines), inner);
}
