use ratatui::layout::{Alignment, Rect};
use ratatui::style::{Color, Modifier, Style};
use ratatui::text::{Line, Span};
use ratatui::widgets::Paragraph;
use ratatui::Frame;

use crate::tui::state::{FocusPanel, TreeItem, TuiState};

pub fn draw_footer(frame: &mut Frame, area: Rect, state: &TuiState) {
    let key_style = Style::default()
        .fg(Color::Cyan)
        .add_modifier(Modifier::BOLD);
    let sep_style = Style::default().fg(Color::DarkGray);

    let mut spans = Vec::new();

    if state.logs_maximized {
        spans.push(Span::styled(" Scroll: ", sep_style));
        spans.push(Span::styled("j/k, PgUp/Dn", key_style));
        spans.push(Span::styled(" | ", sep_style));
        spans.push(Span::styled("Top/Bottom: ", sep_style));
        spans.push(Span::styled("g/G", key_style));
        spans.push(Span::styled(" | ", sep_style));
        spans.push(Span::styled("Restore: ", sep_style));
        spans.push(Span::styled("m / Esc", key_style));
        spans.push(Span::styled(" | ", sep_style));
        spans.push(Span::styled("Clear: ", sep_style));
        spans.push(Span::styled("c", key_style));
        spans.push(Span::styled(" | ", sep_style));
        spans.push(Span::styled("Help: ", sep_style));
        spans.push(Span::styled("?", key_style));
    } else {
        match state.focus {
            FocusPanel::Environments => match state.selected_tree_item() {
                Some(TreeItem::Environment { .. }) => {
                    spans.push(Span::styled(" Nav: ", sep_style));
                    spans.push(Span::styled("j/k", key_style));
                    spans.push(Span::styled(" | ", sep_style));
                    spans.push(Span::styled("Toggle: ", sep_style));
                    spans.push(Span::styled("Space/Enter", key_style));
                    spans.push(Span::styled(" | ", sep_style));
                    spans.push(Span::styled("Join: ", sep_style));
                    spans.push(Span::styled("J", key_style));
                    spans.push(Span::styled(" | ", sep_style));
                    spans.push(Span::styled("Deploy: ", sep_style));
                    spans.push(Span::styled("u", key_style));
                    spans.push(Span::styled(" | ", sep_style));
                    spans.push(Span::styled("Check: ", sep_style));
                    spans.push(Span::styled("c", key_style));
                    spans.push(Span::styled(" | ", sep_style));
                    spans.push(Span::styled("Edit: ", sep_style));
                    spans.push(Span::styled("e", key_style));
                    spans.push(Span::styled(" | ", sep_style));
                    spans.push(Span::styled("New: ", sep_style));
                    spans.push(Span::styled("n", key_style));
                    spans.push(Span::styled(" | ", sep_style));
                    spans.push(Span::styled("Help: ", sep_style));
                    spans.push(Span::styled("?", key_style));
                }
                Some(TreeItem::Host { .. }) => {
                    spans.push(Span::styled(" Nav: ", sep_style));
                    spans.push(Span::styled("j/k", key_style));
                    spans.push(Span::styled(" | ", sep_style));
                    spans.push(Span::styled("SSH: ", sep_style));
                    spans.push(Span::styled("s", key_style));
                    spans.push(Span::styled(" | ", sep_style));
                    spans.push(Span::styled("Logs: ", sep_style));
                    spans.push(Span::styled("l", key_style));
                    spans.push(Span::styled(" | ", sep_style));
                    spans.push(Span::styled("Edit: ", sep_style));
                    spans.push(Span::styled("e", key_style));
                    spans.push(Span::styled(" | ", sep_style));
                    spans.push(Span::styled("Deploy: ", sep_style));
                    spans.push(Span::styled("u", key_style));
                    spans.push(Span::styled(" | ", sep_style));
                    spans.push(Span::styled("Toggle: ", sep_style));
                    spans.push(Span::styled("Space/Enter", key_style));
                    spans.push(Span::styled(" | ", sep_style));
                    spans.push(Span::styled("Help: ", sep_style));
                    spans.push(Span::styled("?", key_style));
                }
                Some(TreeItem::HostService { .. }) => {
                    spans.push(Span::styled(" Nav: ", sep_style));
                    spans.push(Span::styled("j/k", key_style));
                    spans.push(Span::styled(" | ", sep_style));
                    spans.push(Span::styled("Logs: ", sep_style));
                    spans.push(Span::styled("l", key_style));
                    spans.push(Span::styled(" | ", sep_style));
                    spans.push(Span::styled("Edit: ", sep_style));
                    spans.push(Span::styled("e", key_style));
                    spans.push(Span::styled(" | ", sep_style));
                    spans.push(Span::styled("SSH: ", sep_style));
                    spans.push(Span::styled("s", key_style));
                    spans.push(Span::styled(" | ", sep_style));
                    spans.push(Span::styled("Help: ", sep_style));
                    spans.push(Span::styled("?", key_style));
                }
                None => {
                    spans.push(Span::styled(" Nav: ", sep_style));
                    spans.push(Span::styled("j/k", key_style));
                    spans.push(Span::styled(" | ", sep_style));
                    spans.push(Span::styled("Help: ", sep_style));
                    spans.push(Span::styled("?", key_style));
                }
            },
            FocusPanel::Services => {
                spans.push(Span::styled(" Nav: ", sep_style));
                spans.push(Span::styled("j/k", key_style));
                spans.push(Span::styled(" | ", sep_style));
                spans.push(Span::styled("Logs: ", sep_style));
                spans.push(Span::styled("l", key_style));
                spans.push(Span::styled(" | ", sep_style));
                spans.push(Span::styled("Edit: ", sep_style));
                spans.push(Span::styled("e", key_style));
                spans.push(Span::styled(" | ", sep_style));
                spans.push(Span::styled("New: ", sep_style));
                spans.push(Span::styled("n", key_style));
                spans.push(Span::styled(" | ", sep_style));
                spans.push(Span::styled("Help: ", sep_style));
                spans.push(Span::styled("?", key_style));
            }
            FocusPanel::Secrets => {
                spans.push(Span::styled(" Nav: ", sep_style));
                spans.push(Span::styled("j/k", key_style));
                spans.push(Span::styled(" | ", sep_style));
                spans.push(Span::styled("View: ", sep_style));
                spans.push(Span::styled("Enter / v", key_style));
                spans.push(Span::styled(" | ", sep_style));
                spans.push(Span::styled("Edit: ", sep_style));
                spans.push(Span::styled("e", key_style));
                spans.push(Span::styled(" | ", sep_style));
                spans.push(Span::styled("Set -i: ", sep_style));
                spans.push(Span::styled("i", key_style));
                spans.push(Span::styled(" | ", sep_style));
                spans.push(Span::styled("New: ", sep_style));
                spans.push(Span::styled("n", key_style));
                spans.push(Span::styled(" | ", sep_style));
                spans.push(Span::styled("Help: ", sep_style));
                spans.push(Span::styled("?", key_style));
            }
            FocusPanel::Log => {
                spans.push(Span::styled(" Scroll: ", sep_style));
                spans.push(Span::styled("j/k, PgUp/Dn", key_style));
                spans.push(Span::styled(" | ", sep_style));
                spans.push(Span::styled("Top/Bottom: ", sep_style));
                spans.push(Span::styled("g/G", key_style));
                spans.push(Span::styled(" | ", sep_style));
                spans.push(Span::styled("Maximize: ", sep_style));
                spans.push(Span::styled("m", key_style));
                spans.push(Span::styled(" | ", sep_style));
                spans.push(Span::styled("Clear: ", sep_style));
                spans.push(Span::styled("c", key_style));
                spans.push(Span::styled(" | ", sep_style));
                spans.push(Span::styled("Help: ", sep_style));
                spans.push(Span::styled("?", key_style));
            }
            FocusPanel::Details => {
                spans.push(Span::styled(" Nav: ", sep_style));
                spans.push(Span::styled("j/k", key_style));
                spans.push(Span::styled(" | ", sep_style));
                spans.push(Span::styled("Edit: ", sep_style));
                spans.push(Span::styled("e", key_style));
                spans.push(Span::styled(" | ", sep_style));
                spans.push(Span::styled("Help: ", sep_style));
                spans.push(Span::styled("?", key_style));
            }
            FocusPanel::Status => {
                spans.push(Span::styled(" Check: ", sep_style));
                spans.push(Span::styled("c", key_style));
                spans.push(Span::styled(" | ", sep_style));
                spans.push(Span::styled("Deploy: ", sep_style));
                spans.push(Span::styled("u", key_style));
                spans.push(Span::styled(" | ", sep_style));
                spans.push(Span::styled("Help: ", sep_style));
                spans.push(Span::styled("?", key_style));
            }
        }
    }

    let paragraph = Paragraph::new(Line::from(spans));
    frame.render_widget(paragraph, area);

    let right_spans = vec![
        Span::styled("Focus: ", sep_style),
        Span::styled("Tab / 0-5", key_style),
        Span::styled(" | ", sep_style),
        Span::styled("Quit: ", sep_style),
        Span::styled("q", key_style),
        Span::raw("  "),
    ];
    let right_widget = Paragraph::new(Line::from(right_spans)).alignment(Alignment::Right);
    frame.render_widget(right_widget, area);
}
